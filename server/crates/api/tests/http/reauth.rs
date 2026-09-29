use axum::http::StatusCode;
use proto::v1;
use sqlx::PgPool;

use crate::support::{PASSWORD, TestApp, TestRequest, code_in};

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn a_stale_session_reauthenticates_before_a_sensitive_change(pool: PgPool) {
    let app = TestApp::new(pool);
    let token = app.register_verified("alice@example.com").await;
    app.clock.advance(time::Duration::minutes(11));

    let add_app = || TestRequest::post("/api/v1/me/mfa/totp").session(&token);
    app.send(add_app())
        .await
        .assert_problem(StatusCode::FORBIDDEN, "reauth_required");

    let methods = app
        .send(TestRequest::get("/api/v1/me/reauthenticate").session(&token))
        .await;
    assert_eq!(
        methods
            .decode::<v1::ReauthMethods>()
            .methods()
            .collect::<Vec<_>>(),
        [v1::ReauthMethod::Password, v1::ReauthMethod::EmailCode]
    );

    let reauthenticate = |password: &str| {
        TestRequest::post("/api/v1/me/reauthenticate")
            .session(&token)
            .proto(&v1::ReauthenticateRequest {
                method: v1::ReauthMethod::Password.into(),
                secret: password.to_owned(),
            })
    };
    app.send(reauthenticate("nope"))
        .await
        .assert_problem(StatusCode::UNPROCESSABLE_ENTITY, "validation_failed");
    let done = app.send(reauthenticate(PASSWORD)).await;
    assert_eq!(done.status, StatusCode::NO_CONTENT, "{}", done.text);

    let added = app.send(add_app()).await;
    assert_eq!(added.status, StatusCode::OK, "{}", added.text);
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn an_emailed_code_reauthenticates_passwordless_accounts_too(pool: PgPool) {
    let app = TestApp::new(pool);
    let token = app.register_verified("alice@example.com").await;
    app.clock.advance(time::Duration::minutes(11));

    let sent = app
        .send(TestRequest::post("/api/v1/me/reauthenticate/email-code").session(&token))
        .await;
    assert_eq!(sent.status, StatusCode::ACCEPTED, "{}", sent.text);
    let mail = app.mail.last_to("alice@example.com").unwrap();
    let code = code_in(&mail.body);

    let done = app
        .send(
            TestRequest::post("/api/v1/me/reauthenticate")
                .session(&token)
                .proto(&v1::ReauthenticateRequest {
                    method: v1::ReauthMethod::EmailCode.into(),
                    secret: code,
                }),
        )
        .await;
    assert_eq!(done.status, StatusCode::NO_CONTENT, "{}", done.text);
    let deleted = app
        .send(
            TestRequest::delete("/api/v1/me")
                .session(&token)
                .proto(&v1::DeleteAccountRequest::default()),
        )
        .await;
    assert_eq!(deleted.status, StatusCode::NO_CONTENT, "{}", deleted.text);
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn passkey_reauthentication_needs_a_passkey(pool: PgPool) {
    let app = TestApp::new(pool);
    let token = app.register_verified("alice@example.com").await;
    app.send(TestRequest::post("/api/v1/me/reauthenticate/passkey/options").session(&token))
        .await
        .assert_problem(StatusCode::NOT_FOUND, "not_found");
    app.send(
        TestRequest::post("/api/v1/me/reauthenticate/passkey")
            .session(&token)
            .proto(&v1::PasskeyAssertionRequest {
                challenge_id: "0190c1a6-0000-7000-8000-000000000000".to_owned(),
                credential_id: vec![0],
                client_data_json: vec![0],
                authenticator_data: vec![0],
                signature: vec![0],
                user_handle: None,
            }),
    )
    .await
    .assert_problem(StatusCode::BAD_REQUEST, "invalid_passkey");
}
