use axum::http::StatusCode;
use domain::{i18n::Message as Text, user::MIN_PASSWORD_LEN};
use proto::v1;
use serde_json::json;
use sqlx::PgPool;
use time::Duration;

use crate::support::{Options, PASSWORD, TestApp, TestRequest, user};

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn register_signs_in_with_a_secure_cookie(pool: PgPool) {
    let app = TestApp::new(pool);

    let response = app
        .send(
            TestRequest::post("/api/v1/auth/register").proto(&v1::RegisterRequest {
                email: "Alice@Example.com".to_owned(),
                username: "Alice".to_owned(),
                password: PASSWORD.to_owned(),
            }),
        )
        .await;

    assert_eq!(response.status, StatusCode::CREATED, "{}", response.text);
    let me: v1::Me = response.decode();
    assert_eq!(user(&me).email, "alice@example.com");
    assert_eq!(user(&me).roles, ["user"]);
    assert_eq!(
        me.permissions().collect::<Vec<_>>(),
        crate::support::default_permissions()
    );
    let cookie = response.session_cookie().unwrap();
    for attribute in ["HttpOnly", "SameSite=Lax", "Path=/", "Max-Age=2592000"] {
        assert!(cookie.contains(attribute), "{cookie}");
    }
    assert_eq!(response.session_token().unwrap().len(), 43);

    let me = app.me(&response.session_token().unwrap()).await;
    assert_eq!(user(&me).username, "alice");
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn secure_cookies_use_the_host_prefix(pool: PgPool) {
    let app = TestApp::with(
        pool,
        Options {
            secure_cookies: true,
            ..Options::default()
        },
    );

    let token = app.register("alice@example.com").await;
    let response = app.login("alice@example.com", PASSWORD).await;

    let cookie = response.session_cookie().unwrap();
    assert!(cookie.starts_with("__Host-session="), "{cookie}");
    assert!(cookie.contains("Secure"), "{cookie}");
    app.me(&token).await;
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn register_conflicts_and_validation(pool: PgPool) {
    let app = TestApp::new(pool);
    app.register("alice@example.com").await;

    let taken = app
        .send(
            TestRequest::post("/api/v1/auth/register").proto(&v1::RegisterRequest {
                email: "ALICE@example.com".to_owned(),
                username: "alice2".to_owned(),
                password: PASSWORD.to_owned(),
            }),
        )
        .await;
    taken.assert_problem(StatusCode::CONFLICT, "email_taken");

    let taken = app
        .send(
            TestRequest::post("/api/v1/auth/register").proto(&v1::RegisterRequest {
                email: "bob@example.com".to_owned(),
                username: "ALICE".to_owned(),
                password: PASSWORD.to_owned(),
            }),
        )
        .await;
    taken.assert_problem(StatusCode::UNPROCESSABLE_ENTITY, "validation_failed");
    assert_eq!(taken.body["errors"][0]["code"], "username_taken");

    let invalid = app
        .send(
            TestRequest::post("/api/v1/auth/register").proto(&v1::RegisterRequest {
                email: "nope".to_owned(),
                username: String::new(),
                password: "short".to_owned(),
            }),
        )
        .await;
    invalid.assert_problem(StatusCode::UNPROCESSABLE_ENTITY, "validation_failed");
    assert_eq!(
        invalid.body["errors"],
        json!([
            { "field": "email", "code": "invalid_email", "message": app.text("validation-invalid-email") },
            { "field": "username", "code": "required", "message": app.text("validation-required") },
            {
                "field": "password",
                "code": "too_short",
                "message": app.say(&Text::new("validation-password-too-short").arg("min", MIN_PASSWORD_LEN)),
            },
        ])
    );
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn wrong_password_and_unknown_email_get_identical_responses(pool: PgPool) {
    let app = TestApp::new(pool);
    app.register("alice@example.com").await;

    let wrong_password = app.login("alice@example.com", "not the password").await;
    let unknown_email = app.login("nobody@example.com", PASSWORD).await;

    wrong_password.assert_problem(StatusCode::UNAUTHORIZED, "invalid_credentials");
    assert_eq!(wrong_password.text, unknown_email.text);
    assert_eq!(wrong_password.status, unknown_email.status);
    assert!(unknown_email.session_cookie().is_none());
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn login_replaces_the_session_the_browser_had(pool: PgPool) {
    let app = TestApp::new(pool);
    let first = app.register("alice@example.com").await;

    let response = app
        .send(
            TestRequest::post("/api/v1/auth/login")
                .session(&first)
                .proto(&v1::LoginRequest {
                    identifier: "alice@example.com".to_owned(),
                    password: PASSWORD.to_owned(),
                }),
        )
        .await;

    assert_eq!(response.status, StatusCode::OK);
    let second = response.session_token().unwrap();
    assert_ne!(first, second);
    let old = app
        .send(TestRequest::get("/api/v1/me").session(&first))
        .await;
    old.assert_problem(StatusCode::UNAUTHORIZED, "unauthenticated");
    app.me(&second).await;
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn me_requires_a_session(pool: PgPool) {
    let app = TestApp::new(pool);

    let anonymous = app.send(TestRequest::get("/api/v1/me")).await;
    anonymous.assert_problem(StatusCode::UNAUTHORIZED, "unauthenticated");
    assert!(anonymous.session_cookie().is_none());

    let stale = app
        .send(TestRequest::get("/api/v1/me").session("revoked-or-made-up"))
        .await;
    stale.assert_problem(StatusCode::UNAUTHORIZED, "unauthenticated");
    assert!(stale.clears_session());
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn logout_ends_the_session_and_clears_the_cookie(pool: PgPool) {
    let app = TestApp::new(pool);
    let token = app.register("alice@example.com").await;

    let response = app
        .send(TestRequest::post("/api/v1/auth/logout").session(&token))
        .await;
    assert_eq!(response.status, StatusCode::NO_CONTENT);
    assert!(response.clears_session());

    let after = app
        .send(TestRequest::get("/api/v1/me").session(&token))
        .await;
    assert_eq!(after.status, StatusCode::UNAUTHORIZED);

    let again = app.send(TestRequest::post("/api/v1/auth/logout")).await;
    assert_eq!(again.status, StatusCode::NO_CONTENT);
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn logout_all_ends_every_session(pool: PgPool) {
    let app = TestApp::new(pool);
    let first = app.register("alice@example.com").await;
    let second = app
        .login("alice@example.com", PASSWORD)
        .await
        .session_token()
        .unwrap();

    let response = app
        .send(TestRequest::post("/api/v1/auth/logout-all").session(&first))
        .await;
    assert_eq!(response.status, StatusCode::NO_CONTENT);
    assert!(response.clears_session());

    for token in [first, second] {
        let me = app
            .send(TestRequest::get("/api/v1/me").session(&token))
            .await;
        assert_eq!(me.status, StatusCode::UNAUTHORIZED);
    }

    let anonymous = app.send(TestRequest::post("/api/v1/auth/logout-all")).await;
    anonymous.assert_problem(StatusCode::UNAUTHORIZED, "unauthenticated");
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn idle_sessions_expire(pool: PgPool) {
    let app = TestApp::new(pool);
    let token = app.register("alice@example.com").await;

    app.clock.advance(Duration::days(6));
    app.me(&token).await;
    app.clock.advance(Duration::days(7) + Duration::seconds(1));

    let response = app
        .send(TestRequest::get("/api/v1/me").session(&token))
        .await;
    response.assert_problem(StatusCode::UNAUTHORIZED, "unauthenticated");
    assert!(response.clears_session());
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn email_verification_can_be_required(pool: PgPool) {
    let app = TestApp::with(
        pool,
        Options {
            require_email_verification: true,
            ..Options::default()
        },
    );

    let response = app
        .send(
            TestRequest::post("/api/v1/auth/register").proto(&v1::RegisterRequest {
                email: "alice@example.com".to_owned(),
                username: "alice".to_owned(),
                password: PASSWORD.to_owned(),
            }),
        )
        .await;
    assert_eq!(response.status, StatusCode::ACCEPTED, "{}", response.text);
    assert_eq!(
        response.decode::<v1::VerificationPending>().email,
        "alice@example.com"
    );
    assert!(response.session_cookie().is_none());
    let mark = response
        .cookie("registration")
        .expect("the registering browser is marked");

    app.login("alice@example.com", PASSWORD)
        .await
        .assert_problem(StatusCode::FORBIDDEN, "email_not_verified");

    let token = app.mailed_token("alice@example.com");
    let verified = app
        .send(
            TestRequest::post("/api/v1/auth/verify-email")
                .cookie("registration", &mark)
                .proto(&v1::VerifyEmailRequest { token }),
        )
        .await;
    assert_eq!(verified.status, StatusCode::NO_CONTENT);
    assert_eq!(verified.cookie("registration").as_deref(), Some(""));

    let login = app.login("alice@example.com", PASSWORD).await;
    assert_eq!(login.status, StatusCode::OK);
    assert!(user(&login.decode()).email_verified);

    let again = app
        .send(
            TestRequest::post("/api/v1/auth/register").proto(&v1::RegisterRequest {
                email: "Alice@example.com".to_owned(),
                username: "someone".to_owned(),
                password: PASSWORD.to_owned(),
            }),
        )
        .await;
    assert_eq!(again.status, StatusCode::ACCEPTED, "{}", again.text);
    assert_eq!(
        again.decode::<v1::VerificationPending>().email,
        "alice@example.com"
    );
    assert_eq!(
        app.mail.last_to("alice@example.com").unwrap().template,
        "already_registered"
    );
    assert_eq!(
        again.cookie("registration").map(|decoy| decoy.len()),
        Some(mark.len())
    );
}

/// Pre-registration: whoever registers an address first must not keep a password once the owner
/// proves the address from another browser.
#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn a_password_set_before_the_first_proof_does_not_survive_it(pool: PgPool) {
    let app = TestApp::new(pool);
    app.register("victim@example.com").await;

    let token = app.mailed_token("victim@example.com");
    let verified = app
        .send(
            TestRequest::post("/api/v1/auth/verify-email").proto(&v1::VerifyEmailRequest { token }),
        )
        .await;
    assert_eq!(verified.status, StatusCode::NO_CONTENT);

    app.login("victim@example.com", PASSWORD)
        .await
        .assert_problem(StatusCode::UNAUTHORIZED, "invalid_credentials");
    assert_eq!(
        app.mail.last_to("victim@example.com").unwrap().template,
        "choose_password"
    );
}

#[tokio::test]
async fn background_work_is_awaited_on_drain() {
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };

    let background = api::background::Background::spawning();
    let done = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&done);
    background
        .run(async move {
            tokio::task::yield_now().await;
            flag.store(true, Ordering::SeqCst);
        })
        .await;
    background.drain().await;
    assert!(done.load(Ordering::SeqCst));
    background.drain().await;
}
