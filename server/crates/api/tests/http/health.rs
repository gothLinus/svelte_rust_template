use axum::http::StatusCode;
use serde_json::json;
use sqlx::PgPool;

use crate::support::{TestApp, TestRequest};

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn live_and_ready(pool: PgPool) {
    let app = TestApp::new(pool);

    for path in ["/health/live", "/health/ready"] {
        let response = app.send(TestRequest::get(path)).await;
        assert_eq!(response.status, StatusCode::OK, "{path}");
        assert_eq!(response.body, json!({ "status": "ok" }));
    }
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn not_ready_without_a_database(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    pool.close().await;

    let ready = app.send(TestRequest::get("/health/ready")).await;
    assert_eq!(ready.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(ready.body, json!({ "status": "unavailable" }));

    // Liveness does not depend on the database: restarting would not help.
    let live = app.send(TestRequest::get("/health/live")).await;
    assert_eq!(live.status, StatusCode::OK);
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn not_ready_without_the_object_store(pool: PgPool) {
    let app = TestApp::new(pool);
    app.objects.set_down(true);

    let ready = app.send(TestRequest::get("/health/ready")).await;
    assert_eq!(ready.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(ready.body, json!({ "status": "unavailable" }));
    let live = app.send(TestRequest::get("/health/live")).await;
    assert_eq!(live.status, StatusCode::OK);
}
