use std::{num::NonZeroU32, time::Duration};

use api::rate_limit::{Rate, Rates};
use axum::http::StatusCode;
use proto::v1;
use serde_json::json;
use sqlx::PgPool;

use crate::support::{Options, PASSWORD, TestApp, TestRequest};

fn login_body() -> v1::LoginRequest {
    v1::LoginRequest {
        identifier: "alice@example.com".to_owned(),
        password: PASSWORD.to_owned(),
    }
}

fn note(title: &str) -> v1::CreateNoteRequest {
    v1::CreateNoteRequest {
        title: title.to_owned(),
        body: None,
    }
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn state_changing_requests_need_the_custom_header(pool: PgPool) {
    let app = TestApp::new(pool);
    app.register("alice@example.com").await;

    let without = app
        .send(
            TestRequest::post("/api/v1/auth/login")
                .without_csrf_headers()
                .proto(&login_body()),
        )
        .await;
    without.assert_problem(StatusCode::FORBIDDEN, "csrf_rejected");

    // Non-browser clients only need the header; they send no Origin.
    let header_only = app
        .send(
            TestRequest::post("/api/v1/auth/login")
                .without_csrf_headers()
                .header("x-requested-with", "curl")
                .proto(&login_body()),
        )
        .await;
    assert_eq!(header_only.status, StatusCode::OK);
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn foreign_origins_are_rejected(pool: PgPool) {
    let app = TestApp::new(pool);
    let token = app.register("alice@example.com").await;

    for origin in ["https://evil.example", "null", "http://localhost:5174"] {
        let response = app
            .send(
                TestRequest::post("/api/v1/notes")
                    .without_csrf_headers()
                    .header("x-requested-with", "fetch")
                    .header("origin", origin)
                    .session(&token)
                    .proto(&note("forged")),
            )
            .await;
        response.assert_problem(StatusCode::FORBIDDEN, "csrf_rejected");
    }
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn safe_methods_need_no_csrf_headers(pool: PgPool) {
    let app = TestApp::new(pool);
    let token = app.register("alice@example.com").await;
    let response = app
        .send(
            TestRequest::get("/api/v1/me")
                .session(&token)
                .header("origin", "https://evil.example"),
        )
        .await;
    assert_eq!(response.status, StatusCode::OK);
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn api_responses_carry_security_headers(pool: PgPool) {
    let app = TestApp::new(pool);
    let response = app.send(TestRequest::get("/api/v1/me")).await;

    assert_eq!(
        response.header("content-security-policy"),
        Some("default-src 'none'; frame-ancestors 'none'")
    );
    assert_eq!(response.header("x-content-type-options"), Some("nosniff"));
    assert_eq!(response.header("x-frame-options"), Some("DENY"));
    assert_eq!(
        response.header("referrer-policy"),
        Some("strict-origin-when-cross-origin")
    );
    assert_eq!(response.header("cache-control"), Some("no-store"));
    assert!(response.header("strict-transport-security").is_none());
    assert_eq!(response.header("x-request-id").map(str::len), Some(36));
    assert!(response.header("access-control-allow-origin").is_none());
    assert!(
        response
            .header("content-security-policy-report-only")
            .is_none()
    );
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn csp_reports_are_accepted_in_both_formats(pool: PgPool) {
    let app = TestApp::new(pool);
    let legacy = serde_json::to_vec(&json!({
        "csp-report": {
            "document-uri": "http://localhost:5173/",
            "violated-directive": "style-src-elem",
            "blocked-uri": "inline",
        }
    }))
    .unwrap();
    let reporting_api = serde_json::to_vec(&json!([{
        "type": "csp-violation",
        "body": {
            "documentURL": "http://localhost:5173/",
            "effectiveDirective": "require-trusted-types-for",
            "disposition": "report",
            "sample": "Element innerHTML|\n<img src=x onerror=alert(1)>",
        },
    }]))
    .unwrap();

    // Browsers send neither the CSRF header nor the API's content type.
    for (content_type, body) in [
        ("application/csp-report", legacy),
        ("application/reports+json", reporting_api),
        ("application/reports+json", b"not json".to_vec()),
    ] {
        let response = app
            .send(
                TestRequest::post("/csp-reports")
                    .without_csrf_headers()
                    .body(content_type, body),
            )
            .await;
        assert_eq!(response.status, StatusCode::NO_CONTENT, "{content_type}");
    }
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn csp_reports_are_bounded(pool: PgPool) {
    let app = TestApp::with(
        pool,
        Options {
            rates: Some(Rates {
                report_per_ip: Rate::new(NonZeroU32::new(2).unwrap(), Duration::from_mins(1)),
                ..Rates::default()
            }),
            ..Options::default()
        },
    );
    let report = || {
        TestRequest::post("/csp-reports")
            .without_csrf_headers()
            .with_peer("198.51.100.1")
            .body("application/csp-report", "{}")
    };

    let oversized = app
        .send(
            TestRequest::post("/csp-reports")
                .without_csrf_headers()
                .body("application/csp-report", vec![b' '; 32 * 1024]),
        )
        .await;
    assert_eq!(oversized.status, StatusCode::PAYLOAD_TOO_LARGE);

    for _ in 0..2 {
        assert_eq!(app.send(report()).await.status, StatusCode::NO_CONTENT);
    }
    app.send(report())
        .await
        .assert_problem(StatusCode::TOO_MANY_REQUESTS, "rate_limited");
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn request_ids_are_propagated(pool: PgPool) {
    let app = TestApp::new(pool);
    let response = app
        .send(TestRequest::get("/health/live").header("x-request-id", "trace-me-123"))
        .await;
    assert_eq!(response.header("x-request-id"), Some("trace-me-123"));
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn hsts_when_configured(pool: PgPool) {
    let app = TestApp::with(
        pool,
        Options {
            hsts_max_age: Some(Duration::from_hours(365 * 24)),
            ..Options::default()
        },
    );
    let response = app.send(TestRequest::get("/health/live")).await;
    assert_eq!(
        response.header("strict-transport-security"),
        Some("max-age=31536000; includeSubDomains")
    );
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn cors_only_for_configured_origins(pool: PgPool) {
    const OTHER: &str = "https://admin.example.com";
    let app = TestApp::with(
        pool,
        Options {
            cors_origins: vec![OTHER],
            ..Options::default()
        },
    );

    let preflight = app
        .send(
            TestRequest::new(axum::http::Method::OPTIONS, "/api/v1/notes")
                .header("origin", OTHER)
                .header("access-control-request-method", "POST")
                .header(
                    "access-control-request-headers",
                    "content-type,x-requested-with",
                ),
        )
        .await;
    assert_eq!(preflight.header("access-control-allow-origin"), Some(OTHER));
    assert_eq!(
        preflight.header("access-control-allow-credentials"),
        Some("true")
    );

    let foreign = app
        .send(TestRequest::get("/api/v1/me").header("origin", "https://evil.example"))
        .await;
    assert!(foreign.header("access-control-allow-origin").is_none());

    let token = app.register("alice@example.com").await;
    let created = app
        .send(
            TestRequest::post("/api/v1/notes")
                .without_csrf_headers()
                .header("x-requested-with", "fetch")
                .header("origin", OTHER)
                .session(&token)
                .proto(&note("from the admin app")),
        )
        .await;
    assert_eq!(created.status, StatusCode::CREATED);
}

#[test]
fn the_client_ip_is_the_entry_the_first_trusted_proxy_wrote() {
    use api::extract::client_ip;
    use axum::http::HeaderMap;

    let mut headers = HeaderMap::new();
    headers.insert(
        "x-forwarded-for",
        "6.6.6.6, 198.51.100.7, 10.0.0.2".parse().unwrap(),
    );
    let peer = Some("10.0.0.3".parse().unwrap());

    assert_eq!(client_ip(&headers, peer, 0), peer);
    assert_eq!(
        client_ip(&headers, peer, 1),
        Some("10.0.0.2".parse().unwrap())
    );
    assert_eq!(
        client_ip(&headers, peer, 2),
        Some("198.51.100.7".parse().unwrap())
    );
    assert_eq!(client_ip(&headers, peer, 4), peer);

    headers.insert(
        "x-forwarded-for",
        "203.0.113.9:51234, [2001:db8::7]:443".parse().unwrap(),
    );
    assert_eq!(
        client_ip(&headers, peer, 1),
        Some("2001:db8::7".parse().unwrap())
    );
    assert_eq!(
        client_ip(&headers, peer, 2),
        Some("203.0.113.9".parse().unwrap())
    );
}
