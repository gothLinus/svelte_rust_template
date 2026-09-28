use api::problem::ApiError;
use application::AppError;
use axum::{http::StatusCode, response::IntoResponse};
use proto::v1;
use serde_json::json;
use sqlx::PgPool;

use crate::support::{Options, PASSWORD, PROTOBUF, TestApp, TestRequest};

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn problem_documents_have_the_standard_members(pool: PgPool) {
    let app = TestApp::new(pool);

    let response = app.send(TestRequest::get("/api/v1/nothing-here")).await;

    response.assert_problem(StatusCode::NOT_FOUND, "not_found");
    assert_eq!(response.body["type"], "about:blank");
    assert_eq!(response.body["title"], app.text("http-status-404"));
    assert_eq!(response.body["detail"], app.text("error-not-found"));
    assert!(response.body.get("errors").is_none());
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn unknown_api_paths_and_versions_are_404(pool: PgPool) {
    let app = TestApp::new(pool);
    for path in ["/api", "/api/v2/notes", "/api/v1", "/elsewhere"] {
        app.send(TestRequest::get(path))
            .await
            .assert_problem(StatusCode::NOT_FOUND, "not_found");
    }
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn wrong_methods_are_405(pool: PgPool) {
    let app = TestApp::new(pool);
    app.send(TestRequest::delete("/api/v1/auth/login"))
        .await
        .assert_problem(StatusCode::METHOD_NOT_ALLOWED, "method_not_allowed");
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn bodies_must_be_protobuf(pool: PgPool) {
    let app = TestApp::new(pool);
    let login = || TestRequest::post("/api/v1/auth/login");

    app.send(login().json(&json!({ "identifier": "a@example.com", "password": PASSWORD })))
        .await
        .assert_problem(StatusCode::UNSUPPORTED_MEDIA_TYPE, "unsupported_media_type");
    app.send(login().body("text/plain", "hello"))
        .await
        .assert_problem(StatusCode::UNSUPPORTED_MEDIA_TYPE, "unsupported_media_type");
    app.send(login())
        .await
        .assert_problem(StatusCode::UNSUPPORTED_MEDIA_TYPE, "unsupported_media_type");
    let registered = app
        .send(TestRequest::post("/api/v1/auth/register").body(
            "Application/X-Protobuf; charset=binary",
            proto::Message::encode_to_vec(&v1::RegisterRequest {
                email: "alice@example.com".to_owned(),
                username: "alice".to_owned(),
                password: PASSWORD.to_owned(),
            }),
        ))
        .await;
    assert_eq!(
        registered.status,
        StatusCode::CREATED,
        "{}",
        registered.text
    );
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn undecodable_bodies_are_400(pool: PgPool) {
    let app = TestApp::new(pool);
    let login =
        |bytes: &'static [u8]| TestRequest::post("/api/v1/auth/login").body(PROTOBUF, bytes);

    for garbage in [&[0x08, 0xff][..], &[0x0f, 0x01], &[0x0a, 0x02, 0xc3, 0x28]] {
        app.send(login(garbage))
            .await
            .assert_problem(StatusCode::BAD_REQUEST, "invalid_body");
    }
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn unusable_values_are_422_on_the_field(pool: PgPool) {
    let app = TestApp::new(pool);
    let token = app.register_verified("alice@example.com").await;

    let assertion = |challenge_id: &str| {
        TestRequest::post("/api/v1/auth/passkeys/login").proto(&v1::PasskeyAssertionRequest {
            challenge_id: challenge_id.to_owned(),
            credential_id: vec![0; 3],
            client_data_json: b"{}".to_vec(),
            authenticator_data: vec![0; 3],
            signature: vec![0; 3],
            user_handle: None,
        })
    };
    for (challenge_id, code) in [("not-a-uuid", "invalid"), ("", "required")] {
        let response = app.send(assertion(challenge_id)).await;
        response.assert_problem(StatusCode::UNPROCESSABLE_ENTITY, "validation_failed");
        assert_eq!(response.body["errors"][0]["field"], "challengeId");
        assert_eq!(response.body["errors"][0]["code"], code);
    }

    for (channel, code) in [
        (v1::TextChannel::Unspecified as i32, "required"),
        (99, "invalid"),
    ] {
        let response = app
            .send(
                TestRequest::post("/api/v1/auth/phone-code").proto(&v1::PhoneCodeRequest {
                    phone: "+491701234567".to_owned(),
                    channel,
                }),
            )
            .await;
        response.assert_problem(StatusCode::UNPROCESSABLE_ENTITY, "validation_failed");
        assert_eq!(response.body["errors"][0]["field"], "channel");
        assert_eq!(response.body["errors"][0]["code"], code);
    }
    let phone = app
        .send(
            TestRequest::post("/api/v1/me/phone")
                .session(&token)
                .proto(&v1::AddPhoneRequest::default()),
        )
        .await;
    phone.assert_problem(StatusCode::UNPROCESSABLE_ENTITY, "validation_failed");
    assert_eq!(phone.body["errors"][0]["field"], "channel");

    for method in [v1::ReauthMethod::Unspecified, v1::ReauthMethod::Passkey] {
        let response = app
            .send(
                TestRequest::post("/api/v1/me/reauthenticate")
                    .session(&token)
                    .proto(&v1::ReauthenticateRequest {
                        method: method as i32,
                        secret: PASSWORD.to_owned(),
                    }),
            )
            .await;
        response.assert_problem(StatusCode::UNPROCESSABLE_ENTITY, "validation_failed");
        assert_eq!(response.body["errors"][0]["field"], "method");
    }
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn responses_are_protobuf(pool: PgPool) {
    let app = TestApp::new(pool);
    let token = app.register("alice@example.com").await;

    let response = app
        .send(TestRequest::get("/api/v1/me").session(&token))
        .await;
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.header("content-type"), Some(PROTOBUF));
    let me: v1::Me = response.decode();
    assert_eq!(me.user.unwrap().email, "alice@example.com");

    let methods = app.send(TestRequest::get("/api/v1/auth/methods")).await;
    assert_eq!(methods.header("content-type"), Some(PROTOBUF));
}

#[test]
fn secrets_are_redacted_from_debug() {
    let login = v1::LoginRequest {
        identifier: "alice@example.com".to_owned(),
        password: "hunter2-secret".to_owned(),
    };
    let debug = format!("{login:?}");
    assert!(debug.contains("alice@example.com"), "{debug}");
    assert!(!debug.contains("hunter2-secret"), "{debug}");

    let reauth = v1::ReauthenticateRequest {
        method: v1::ReauthMethod::Password as i32,
        secret: "hunter2-secret".to_owned(),
    };
    assert!(!format!("{reauth:?}").contains("hunter2-secret"));
    let setup = v1::TotpSetup {
        secret: "JBSWY3DPEHPK3PXP".to_owned(),
        uri: "otpauth://totp/Acme?secret=JBSWY3DPEHPK3PXP".to_owned(),
    };
    assert!(!format!("{setup:?}").contains("JBSWY3DPEHPK3PXP"));
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn oversized_bodies_are_413(pool: PgPool) {
    let app = TestApp::with(
        pool,
        Options {
            max_body_bytes: 1024,
            ..Options::default()
        },
    );

    let response = app
        .send(
            TestRequest::post("/api/v1/auth/login").proto(&v1::LoginRequest {
                identifier: "a@example.com".to_owned(),
                password: "x".repeat(2048),
            }),
        )
        .await;

    assert_eq!(
        response.status,
        StatusCode::PAYLOAD_TOO_LARGE,
        "{}",
        response.text
    );
    assert_eq!(
        response.header("content-type"),
        Some("application/problem+json")
    );
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn database_failures_are_500s_without_details(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    let token = app.register("alice@example.com").await;
    pool.close().await;

    let response = app
        .send(TestRequest::get("/api/v1/me").session(&token))
        .await;

    response.assert_problem(StatusCode::INTERNAL_SERVER_ERROR, "internal_error");
    assert_eq!(response.body["detail"], app.text("error-internal"));
    assert!(!response.text.to_lowercase().contains("pool"));
}

#[test]
fn busy_is_a_503_with_retry_after() {
    let response = ApiError::from(AppError::Busy).into_response();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(response.headers()["retry-after"], "1");
}

#[test]
fn problem_documents_omit_empty_members() {
    use api::problem::{ProblemDetails, ProblemField};

    let problem = ProblemDetails {
        kind: "about:blank".to_owned(),
        title: "title".to_owned(),
        status: 404,
        detail: None,
        instance: None,
        code: "not_found".to_owned(),
        errors: None,
    };
    assert_eq!(
        serde_json::to_value(problem).unwrap(),
        json!({ "type": "about:blank", "title": "title", "status": 404, "code": "not_found" })
    );
    let field = ProblemField {
        field: "newPassword".to_owned(),
        code: "too_short".to_owned(),
        message: "message".to_owned(),
    };
    assert_eq!(
        serde_json::to_value(field).unwrap(),
        json!({ "field": "newPassword", "code": "too_short", "message": "message" })
    );
}
