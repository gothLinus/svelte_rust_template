use std::sync::Arc;

use axum::http::StatusCode;
use proto::v1;
use sqlx::PgPool;

use crate::support::{PASSWORD, TestApp, TestRequest};

async fn burst(app: &Arc<TestApp>, requests: Vec<TestRequest>) -> Vec<StatusCode> {
    let tasks: Vec<_> = requests
        .into_iter()
        .map(|request| {
            let app = Arc::clone(app);
            tokio::spawn(async move { app.send(request).await.status })
        })
        .collect();
    let mut statuses = Vec::with_capacity(tasks.len());
    for task in tasks {
        statuses.push(task.await.unwrap());
    }
    statuses
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn parallel_second_step_guesses_get_five_evaluations(pool: PgPool) {
    let app = Arc::new(TestApp::new(pool));
    let token = app.register_verified("alice@example.com").await;
    app.enable_totp(&token).await;
    let attempt = app
        .login("alice@example.com", PASSWORD)
        .await
        .cookie("mfa")
        .unwrap();

    let guesses = (0..40)
        .map(|n| {
            TestRequest::post("/api/v1/auth/mfa/totp")
                .cookie("mfa", &attempt)
                .proto(&v1::CodeRequest {
                    code: format!("{n:06}"),
                })
        })
        .collect();
    let statuses = burst(&app, guesses).await;

    let evaluated = statuses
        .iter()
        .filter(|status| **status == StatusCode::UNPROCESSABLE_ENTITY)
        .count();
    let refused = statuses
        .iter()
        .filter(|status| **status == StatusCode::CONFLICT)
        .count();
    assert!(evaluated <= 5, "{evaluated} guesses were evaluated");
    assert_eq!(evaluated + refused, 40, "{statuses:?}");
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn parallel_code_guesses_cannot_exceed_the_attempt_limit(pool: PgPool) {
    let app = Arc::new(TestApp::new(pool));
    app.register("alice@example.com").await;
    let sent = app
        .send(
            TestRequest::post("/api/v1/auth/email-code").proto(&v1::EmailCodeRequest {
                email: "alice@example.com".to_owned(),
            }),
        )
        .await;
    assert_eq!(sent.status, StatusCode::ACCEPTED);

    let guesses = (0..40)
        .map(|n| {
            TestRequest::post("/api/v1/auth/email-code/verify").proto(&v1::VerifyEmailCodeRequest {
                email: "alice@example.com".to_owned(),
                code: format!("{n:06}"),
            })
        })
        .collect();
    burst(&app, guesses).await;

    let attempts: i32 = sqlx::query_scalar("select attempts from one_time_codes")
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(attempts, 5);
    let code = app.mailed_code("alice@example.com");
    app.send(TestRequest::post("/api/v1/auth/email-code/verify").proto(
        &v1::VerifyEmailCodeRequest {
            email: "alice@example.com".to_owned(),
            code,
        },
    ))
    .await
    .assert_invalid_code();
}
