use axum::http::StatusCode;
use proto::v1;
use sqlx::PgPool;
use time::Duration;

use crate::support::{PASSWORD, TestApp, TestRequest, user};

fn forgot(email: &str) -> v1::ForgotPasswordRequest {
    v1::ForgotPasswordRequest {
        email: email.to_owned(),
    }
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn verifying_the_email_address(pool: PgPool) {
    let app = TestApp::new(pool);
    let session = app.register("alice@example.com").await;
    let mail = app.mail.last_to("alice@example.com").unwrap();
    assert!(
        mail.body
            .contains("http://localhost:5173/verify-email#token=")
    );
    let token = app.mailed_token("alice@example.com");
    assert!(!user(&app.me(&session).await).email_verified);

    let elsewhere = app
        .login("alice@example.com", PASSWORD)
        .await
        .session_token()
        .unwrap();

    let verify = || {
        TestRequest::post("/api/v1/auth/verify-email")
            .session(&session)
            .proto(&v1::VerifyEmailRequest {
                token: token.clone(),
            })
    };
    assert_eq!(app.send(verify()).await.status, StatusCode::NO_CONTENT);
    assert!(user(&app.me(&session).await).email_verified);
    app.send(TestRequest::get("/api/v1/me").session(&elsewhere))
        .await
        .assert_problem(StatusCode::UNAUTHORIZED, "unauthenticated");

    app.send(verify())
        .await
        .assert_problem(StatusCode::BAD_REQUEST, "invalid_token");
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn resending_the_verification_link(pool: PgPool) {
    let app = TestApp::new(pool);
    let session = app.register("alice@example.com").await;
    let first = app.mailed_token("alice@example.com");

    let resend = || TestRequest::post("/api/v1/auth/verify-email/resend").session(&session);
    assert_eq!(app.send(resend()).await.status, StatusCode::NO_CONTENT);
    let second = app.mailed_token("alice@example.com");
    assert_ne!(first, second);

    app.send(
        TestRequest::post("/api/v1/auth/verify-email")
            .proto(&v1::VerifyEmailRequest { token: first }),
    )
    .await
    .assert_problem(StatusCode::BAD_REQUEST, "invalid_token");
    app.send(
        TestRequest::post("/api/v1/auth/verify-email")
            .session(&session)
            .proto(&v1::VerifyEmailRequest { token: second }),
    )
    .await;
    app.send(resend())
        .await
        .assert_problem(StatusCode::CONFLICT, "already_verified");

    app.send(TestRequest::post("/api/v1/auth/verify-email/resend"))
        .await
        .assert_problem(StatusCode::UNAUTHORIZED, "unauthenticated");
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn forgot_password_answers_the_same_for_every_address(pool: PgPool) {
    let app = TestApp::new(pool);
    app.register("alice@example.com").await;
    app.mail.take();

    let known = app
        .send(TestRequest::post("/api/v1/auth/forgot-password").proto(&forgot("alice@example.com")))
        .await;
    let unknown = app
        .send(
            TestRequest::post("/api/v1/auth/forgot-password").proto(&forgot("nobody@example.com")),
        )
        .await;

    assert_eq!(known.status, StatusCode::ACCEPTED);
    assert_eq!(unknown.status, StatusCode::ACCEPTED);
    assert_eq!(known.text, unknown.text);
    let sent = app.mail.take();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].to.as_str(), "alice@example.com");
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn resetting_the_password_signs_out_everywhere(pool: PgPool) {
    let app = TestApp::new(pool);
    let session = app.register("alice@example.com").await;
    app.send(TestRequest::post("/api/v1/auth/forgot-password").proto(&forgot("alice@example.com")))
        .await;
    let token = app.mailed_token("alice@example.com");

    let reset = |password: &str| {
        TestRequest::post("/api/v1/auth/reset-password").proto(&v1::ResetPasswordRequest {
            token: token.clone(),
            password: password.to_owned(),
        })
    };
    app.send(reset("short"))
        .await
        .assert_problem(StatusCode::UNPROCESSABLE_ENTITY, "validation_failed");
    assert_eq!(
        app.send(reset("a brand new password")).await.status,
        StatusCode::NO_CONTENT
    );

    let old_session = app
        .send(TestRequest::get("/api/v1/me").session(&session))
        .await;
    assert_eq!(old_session.status, StatusCode::UNAUTHORIZED);
    app.login("alice@example.com", PASSWORD)
        .await
        .assert_problem(StatusCode::UNAUTHORIZED, "invalid_credentials");
    assert_eq!(
        app.login("alice@example.com", "a brand new password")
            .await
            .status,
        StatusCode::OK
    );
    assert_eq!(
        app.mail.last_to("alice@example.com").unwrap().subject,
        "Your password was changed"
    );

    app.send(reset("yet another password"))
        .await
        .assert_problem(StatusCode::BAD_REQUEST, "invalid_token");
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn reset_links_expire(pool: PgPool) {
    let app = TestApp::new(pool);
    app.register("alice@example.com").await;
    app.send(TestRequest::post("/api/v1/auth/forgot-password").proto(&forgot("alice@example.com")))
        .await;
    let token = app.mailed_token("alice@example.com");

    app.clock.advance(Duration::minutes(31));

    app.send(
        TestRequest::post("/api/v1/auth/reset-password").proto(&v1::ResetPasswordRequest {
            token,
            password: "a brand new password".to_owned(),
        }),
    )
    .await
    .assert_problem(StatusCode::BAD_REQUEST, "invalid_token");
}
