//! Hardening headers on every response.
//!
//! [`apply`] sits outside the timeout and panic layers in `crate::router`, so their responses
//! carry the headers too. The Content Security Policy depends on the path: under `/api/` nothing
//! may load at all, everywhere else the SPA's policy applies.

use std::time::Duration;

use axum::{
    extract::{Request, State},
    http::{
        HeaderName, HeaderValue,
        header::{
            CONTENT_SECURITY_POLICY, CONTENT_SECURITY_POLICY_REPORT_ONLY, REFERRER_POLICY,
            STRICT_TRANSPORT_SECURITY, X_CONTENT_TYPE_OPTIONS, X_FRAME_OPTIONS,
        },
    },
    middleware::Next,
    response::Response,
};

/// API responses are protobuf messages or JSON problem documents, never rendered, so they may load
/// nothing at all.
const API_CSP: &str = "default-src 'none'; frame-ancestors 'none'";

/// The SPA gets its main policy from a `<meta>` tag that SvelteKit writes at build time, with
/// hashes of its inline bootstrap script (`kit.csp` in `web/vite.config.ts`). This header adds
/// what a `<meta>` policy cannot carry (`frame-ancestors`, reporting), repeats the most important
/// restrictions, and narrows inline styles: bits-ui positions popovers with `style` attributes,
/// but no `<style>` element may come from anywhere but the build. Browsers enforce both policies.
/// It is sent in production only; the Vite dev server injects `<style>` elements for hot
/// reloading.
const SPA_CSP: &str = "frame-ancestors 'none'; object-src 'none'; base-uri 'self'; \
     form-action 'self'; style-src-elem 'self'; style-src-attr 'unsafe-inline'; \
     report-to csp; report-uri /csp-reports";

/// Trusted Types, reported but not enforced: Svelte renders its templates through its own
/// `svelte-trusted-html` policy, so the app needs no other, but SvelteKit's last-resort error page
/// is parsed without one. Once the reports stay quiet, move these directives into [`SPA_CSP`] to
/// turn every other HTML sink (`innerHTML`, `eval`, ...) off.
const SPA_CSP_REPORT_ONLY: &str = "require-trusted-types-for 'script'; \
     trusted-types svelte-trusted-html; report-to csp; report-uri /csp-reports";

/// Where the `report-to csp` directive sends violation reports (see `routes::reports`).
const REPORTING_ENDPOINTS: &str = "csp=\"/csp-reports\"";

#[derive(Debug, Clone, Default)]
pub struct SecurityHeaders {
    hsts: Option<HeaderValue>,
}

impl SecurityHeaders {
    /// `hsts_max_age` enables `Strict-Transport-Security` (on by default for an `https://`
    /// `APP_URL`). The header always carries `includeSubDomains`, so every subdomain of the app's
    /// host must be served over HTTPS too: browsers refuse plain HTTP to all of them for that long.
    pub fn new(hsts_max_age: Option<Duration>) -> Self {
        Self {
            hsts: hsts_max_age.map(|max_age| {
                HeaderValue::from_str(&format!("max-age={}; includeSubDomains", max_age.as_secs()))
                    .unwrap_or(HeaderValue::from_static(
                        "max-age=31536000; includeSubDomains",
                    ))
            }),
        }
    }
}

pub async fn apply(
    State(config): State<SecurityHeaders>,
    request: Request,
    next: Next,
) -> Response {
    let is_api = request.uri().path().starts_with("/api/");
    let mut response = next.run(request).await;
    let headers = response.headers_mut();

    if is_api {
        headers.insert(CONTENT_SECURITY_POLICY, HeaderValue::from_static(API_CSP));
    } else {
        headers.insert(CONTENT_SECURITY_POLICY, HeaderValue::from_static(SPA_CSP));
        headers.insert(
            CONTENT_SECURITY_POLICY_REPORT_ONLY,
            HeaderValue::from_static(SPA_CSP_REPORT_ONLY),
        );
        headers.insert(
            HeaderName::from_static("reporting-endpoints"),
            HeaderValue::from_static(REPORTING_ENDPOINTS),
        );
    }
    headers.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    headers.insert(X_FRAME_OPTIONS, HeaderValue::from_static("DENY"));
    headers.insert(
        REFERRER_POLICY,
        HeaderValue::from_static("strict-origin-when-cross-origin"),
    );
    headers.insert(
        HeaderName::from_static("cross-origin-opener-policy"),
        HeaderValue::from_static("same-origin"),
    );
    headers.insert(
        HeaderName::from_static("permissions-policy"),
        HeaderValue::from_static("camera=(), geolocation=(), microphone=(), payment=()"),
    );
    if let Some(hsts) = &config.hsts {
        headers.insert(STRICT_TRANSPORT_SECURITY, hsts.clone());
    }

    response
}
