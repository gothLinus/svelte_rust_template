//! Exported traces, in a test binary of their own: it installs the process-wide subscriber, which
//! the other tests must not share. A thread-local one races with tests on other threads over
//! `tracing`'s callsite cache.

#![expect(
    clippy::unwrap_used,
    reason = "`clippy.toml` only exempts `#[test]` functions; the helpers here may unwrap too"
)]

use std::{
    net::{Ipv4Addr, SocketAddr},
    sync::{Arc, OnceLock},
    time::Duration,
};

use api::{problem::ApiError, router, telemetry};
use axum::{
    Router,
    body::Body,
    http::{Request, StatusCode},
};
use domain::i18n::Translator;
use http_body_util::BodyExt;
use i18n::Catalog;
use infrastructure::config::{HttpConfig, PublicOrigin, RateLimitStore};
use opentelemetry::{global, trace::TracerProvider};
use opentelemetry_sdk::{
    propagation::TraceContextPropagator,
    trace::{InMemorySpanExporter, SdkTracerProvider},
};
use serde_json::Value;
use tower::ServiceExt;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

const TRACE_ID: &str = "4bf92f3577b34da6a3ce929d0e0e4736";
const PARENT_SPAN_ID: &str = "00f067aa0ba902b7";

/// What every test here records into, set up once as the global subscriber.
fn spans() -> &'static InMemorySpanExporter {
    static EXPORTER: OnceLock<InMemorySpanExporter> = OnceLock::new();
    EXPORTER.get_or_init(|| {
        let exporter = InMemorySpanExporter::default();
        let provider = SdkTracerProvider::builder()
            .with_simple_exporter(exporter.clone())
            .build();
        global::set_text_map_propagator(TraceContextPropagator::new());
        tracing_subscriber::registry()
            .with(tracing_opentelemetry::layer().with_tracer(provider.tracer(telemetry::SCOPE)))
            .init();
        exporter
    })
}

fn app() -> Router {
    let config = HttpConfig {
        bind_addr: SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
        public_url: PublicOrigin::parse("http://localhost:5173").unwrap(),
        static_dir: None,
        secure_cookies: false,
        trusted_proxy_hops: 0,
        hsts_max_age: None,
        cors_origins: Vec::new(),
        request_timeout: Duration::from_secs(10),
        upload_timeout: Duration::from_secs(30),
        max_body_bytes: 64 * 1024,
        rate_limits: false,
        rates: api::rate_limit::Rates::default(),
        rate_limit_store: RateLimitStore::Memory,
        rate_limit_memory_max_keys: api::rate_limit::DEFAULT_MEMORY_MAX_KEYS,
        shutdown_timeout: Duration::from_secs(5),
    };
    let translator: Arc<dyn Translator> = Arc::new(Catalog::embedded().unwrap());
    router::with_middleware(
        Router::new().fallback(|| async { ApiError::not_found() }),
        &config,
        translator,
    )
}

#[tokio::test]
async fn a_callers_trace_is_continued_and_named_in_problem_documents() {
    let exporter = spans();
    let request = Request::get("/nowhere")
        .header("traceparent", format!("00-{TRACE_ID}-{PARENT_SPAN_ID}-01"))
        .body(Body::empty())
        .unwrap();

    let response = app().oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let problem: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(problem["traceId"], TRACE_ID);
    let spans = exporter.get_finished_spans().unwrap();
    let request_span = spans
        .iter()
        .find(|span| span.span_context.trace_id().to_string() == TRACE_ID)
        .unwrap();
    assert_eq!(request_span.name, "request");
    assert_eq!(request_span.parent_span_id.to_string(), PARENT_SPAN_ID);
}

#[tokio::test]
async fn a_request_without_a_trace_starts_one() {
    spans();

    let response = app()
        .oneshot(Request::get("/nowhere").body(Body::empty()).unwrap())
        .await
        .unwrap();

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let problem: Value = serde_json::from_slice(&body).unwrap();
    let trace_id = problem["traceId"].as_str().unwrap();
    assert_eq!(trace_id.len(), 32);
    assert_ne!(trace_id, TRACE_ID);
}
