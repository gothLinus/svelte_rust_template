//! Assembles routes and middleware into the application.
//!
//! [`build`] layers it in three tiers, outermost first:
//!
//! 1. Every response: request id, security headers, tracing, compression, the language of problem documents (`middleware::localize`), timeout
//!    (`REQUEST_TIMEOUT`) and panic handling (both answer
//!    with a problem document) and the body size limit.
//! 2. The `/api/v1` group: CORS (only when origins are configured), `Cache-Control: no-store`, the
//!    per-IP rate limit, CSRF protection, then the session lookup. A request over the limit or
//!    without the CSRF header is refused before any database work.
//! 3. The routes: the group's own, `/health/*`, `/csp-reports`, and a JSON `404` for unknown `/api`
//!    paths. With `static_dir` set, anything else is the SPA (see `crate::spa`).

use std::{any::Any, iter, sync::Arc};

use application::Adapters;
use axum::{
    BoxError, Router,
    body::Body,
    error_handling::HandleErrorLayer,
    extract::DefaultBodyLimit,
    http::{HeaderName, HeaderValue, Method, Request, header},
    middleware::from_fn_with_state,
    response::{IntoResponse, Response},
    routing::any,
};
use domain::i18n::Translator;
use infrastructure::config::HttpConfig;
use tower::{ServiceBuilder, timeout::TimeoutLayer, timeout::error::Elapsed};
use tower_http::{
    catch_panic::CatchPanicLayer,
    compression::{
        CompressionLayer, CompressionLevel,
        predicate::{DefaultPredicate, Predicate, SizeAbove},
    },
    cors::{AllowOrigin, CorsLayer},
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, RequestId, SetRequestIdLayer},
    set_header::SetResponseHeaderLayer,
    trace::TraceLayer,
};
use tracing::Span;

use crate::{
    middleware::{
        csrf::{self, TrustedOrigins, X_REQUESTED_WITH},
        localize, rate_limit,
        security_headers::{self, SecurityHeaders},
        session,
    },
    problem::ApiError,
    routes, spa,
    state::AppState,
};

/// For responses compressed as they are sent: API bodies and whatever the build did not
/// precompress. The libraries' default is brotli's maximum, 11, which is many times slower than 4
/// for output only a few percent smaller. The SPA's assets are compressed at the maximum once, at
/// build time (see `spa`).
const COMPRESSION_LEVEL: CompressionLevel = CompressionLevel::Precise(4);

/// Smaller responses go out as they are. Protobuf has no repeated keys to squeeze out, so a small
/// message barely shrinks, and the encoder's CPU and framing cost more than the bytes saved. The
/// libraries' default is 32 bytes.
const COMPRESSION_MIN_BYTES: u64 = 1024;

/// The whole application: `/api/v1/*`, `/health/*`, `/csp-reports`, and (with `static_dir`) the
/// SPA.
pub fn build<A: Adapters>(state: AppState<A>, config: &HttpConfig) -> Router {
    let trusted = TrustedOrigins::new(
        iter::once(&config.public_url)
            .chain(&config.cors_origins)
            .map(ToString::to_string),
    );

    // Layers run outside-in in reverse order of `.layer` calls: CORS, then the per-IP rate limit,
    // then CSRF, then the session lookup, then the handler.
    let mut api = routes::api_v1::<A>()
        .fallback(|| async { ApiError::not_found() })
        .method_not_allowed_fallback(|| async { ApiError::method_not_allowed() })
        .layer(from_fn_with_state(
            state.clone(),
            session::authenticate::<A>,
        ))
        .layer(from_fn_with_state(trusted, csrf::protect))
        .layer(from_fn_with_state(state.clone(), rate_limit::limit::<A>))
        // Responses carry account data; no cache along the way may keep them.
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ));
    if let Some(cors) = cors_layer(config) {
        api = api.layer(cors);
    }

    let translator = Arc::clone(&state.translator);
    let app = Router::new()
        .nest("/api/v1", api)
        .merge(routes::health::<A>())
        .merge(routes::reports::<A>())
        // Unknown API versions and paths are 404s, never the SPA.
        .route("/api", any(|| async { ApiError::not_found() }))
        .route("/api/{*rest}", any(|| async { ApiError::not_found() }))
        .with_state(state);

    let app = match &config.static_dir {
        Some(dir) => app.fallback_service(spa::router(dir)),
        None => app.fallback(|| async { ApiError::not_found() }),
    };

    with_middleware(app, config, translator)
}

/// The layers every response goes through, listed outermost first: every request gets an id
/// before it is traced, every response gets the security headers, and timeouts and panics answer
/// with a problem document like any other error. Problem documents are written by `localize`,
/// inside compression (so they are compressed like any body) and outside the timeout and panic
/// layers (so theirs are localized too). File contents leave uncompressed (`NotFileContents`).
pub fn with_middleware(
    app: Router,
    config: &HttpConfig,
    translator: Arc<dyn Translator>,
) -> Router {
    app.layer(
        ServiceBuilder::new()
            .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
            .layer(PropagateRequestIdLayer::x_request_id())
            .layer(from_fn_with_state(
                SecurityHeaders::new(config.hsts_max_age),
                security_headers::apply,
            ))
            .layer(TraceLayer::new_for_http().make_span_with(request_span))
            .layer(
                CompressionLayer::new()
                    .quality(COMPRESSION_LEVEL)
                    .compress_when(
                        DefaultPredicate::new().and(SizeAbove::new(COMPRESSION_MIN_BYTES)),
                    ),
            )
            .layer(from_fn_with_state(translator, localize::apply))
            .layer(HandleErrorLayer::new(middleware_error))
            .layer(TimeoutLayer::new(config.request_timeout))
            .layer(CatchPanicLayer::custom(panic_response))
            .layer(DefaultBodyLimit::max(config.max_body_bytes)),
    )
}

fn cors_layer(config: &HttpConfig) -> Option<CorsLayer> {
    let origins: Vec<HeaderValue> = config
        .cors_origins
        .iter()
        .filter_map(|origin| HeaderValue::from_str(origin.as_str()).ok())
        .collect();
    if origins.is_empty() {
        return None;
    }

    Some(
        CorsLayer::new()
            .allow_origin(AllowOrigin::list(origins))
            .allow_credentials(true)
            .allow_methods([
                Method::GET,
                Method::POST,
                Method::PUT,
                Method::PATCH,
                Method::DELETE,
            ])
            .allow_headers([header::CONTENT_TYPE, X_REQUESTED_WITH])
            .expose_headers([
                header::RETRY_AFTER,
                header::LOCATION,
                HeaderName::from_static("x-request-id"),
            ])
            .max_age(std::time::Duration::from_hours(1)),
    )
}

fn request_span(request: &Request<Body>) -> Span {
    let request_id = request
        .extensions()
        .get::<RequestId>()
        .and_then(|id| id.header_value().to_str().ok())
        .unwrap_or_default();

    tracing::info_span!(
        "request",
        method = %request.method(),
        path = %request.uri().path(),
        request_id,
    )
}

async fn middleware_error(err: BoxError) -> ApiError {
    if err.is::<Elapsed>() {
        ApiError::timeout()
    } else {
        ApiError::internal(&*err)
    }
}

fn panic_response(_: Box<dyn Any + Send + 'static>) -> Response {
    tracing::error!("request handler panicked");
    ApiError::internal_without_cause().into_response()
}
