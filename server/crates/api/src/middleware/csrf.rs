//! Cross-site request forgery protection for the API.
//!
//! Two independent checks on every state-changing request (anything but GET, HEAD, OPTIONS,
//! TRACE):
//!
//! 1. **A custom header is required**: `X-Requested-With` must be present. An HTML form or an
//!    `<img>` cannot set custom headers, and a cross-origin `fetch` that does triggers a CORS
//!    preflight, which fails unless the origin is explicitly allowed.
//! 2. **The `Origin` must be ours** if the browser sends one (all modern browsers do for non-GET
//!    requests): the public URL, or one of the configured CORS origins.
//!
//! The one exception is the social sign-in callback, which Sign in with Apple posts as a form from
//! its own origin. It changes nothing: it only redirects to the GET callback, and the flow itself
//! is bound to the browser by a cookie and the `state` parameter.
//!
//! On top of that the session cookie is `SameSite=Lax`, which already keeps it off cross-site
//! POSTs, and request bodies are `application/x-protobuf`, a content type an HTML form cannot send
//! and a cross-origin `fetch` cannot send without a preflight. Token-based CSRF schemes add nothing
//! for a same-origin SPA and would need server-side state. Non-browser clients (curl, other
//! servers) are unaffected by CSRF and only need to send the header.

use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::{HeaderName, header::ORIGIN},
    middleware::Next,
    response::Response,
};
use domain::i18n::Message;

use crate::problem::ApiError;

pub const X_REQUESTED_WITH: HeaderName = HeaderName::from_static("x-requested-with");

#[derive(Debug, Clone)]
pub struct TrustedOrigins(Arc<[String]>);

impl TrustedOrigins {
    pub fn new(origins: impl IntoIterator<Item = String>) -> Self {
        Self(origins.into_iter().collect())
    }

    fn allows(&self, origin: &[u8]) -> bool {
        self.0.iter().any(|trusted| trusted.as_bytes() == origin)
    }
}

/// Rejects a state-changing request that lacks the header or comes from an untrusted origin with
/// `403 csrf_rejected`.
pub async fn protect(
    State(origins): State<TrustedOrigins>,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    if !request.method().is_safe() && !is_oauth_callback(request.uri().path()) {
        let headers = request.headers();
        if headers
            .get(X_REQUESTED_WITH)
            .is_none_or(axum::http::HeaderValue::is_empty)
        {
            return Err(ApiError::csrf_rejected(Message::new(
                "http-csrf-header-required",
            )));
        }
        if headers
            .get(ORIGIN)
            .is_some_and(|origin| !origins.allows(origin.as_bytes()))
        {
            return Err(ApiError::csrf_rejected(Message::new(
                "http-csrf-cross-origin",
            )));
        }
    }
    Ok(next.run(request).await)
}

fn is_oauth_callback(path: &str) -> bool {
    let path = path.strip_prefix("/api/v1").unwrap_or(path);
    path.strip_prefix("/auth/oauth/")
        .and_then(|rest| rest.strip_suffix("/callback"))
        .is_some_and(|provider| !provider.is_empty() && !provider.contains('/'))
}
