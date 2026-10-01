use axum::http::StatusCode;
use proto::v1;
use sqlx::PgPool;

use crate::support::{PASSWORD, TestApp, TestRequest};

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn the_own_activity_lists_sign_ins_from_the_requests_client(pool: PgPool) {
    let app = TestApp::new(pool);
    app.register("alice@example.com").await;
    let response = app
        .send(
            TestRequest::post("/api/v1/auth/login")
                .proto(&v1::LoginRequest {
                    identifier: "alice@example.com".to_owned(),
                    password: PASSWORD.to_owned(),
                })
                .with_peer("203.0.113.7")
                .header("user-agent", "Firefox"),
        )
        .await;
    let token = response.session_token().unwrap();

    app.send(TestRequest::get("/api/v1/me/activity"))
        .await
        .assert_problem(StatusCode::UNAUTHORIZED, "unauthenticated");
    let listed = app
        .send(TestRequest::get("/api/v1/me/activity?limit=2").session(&token))
        .await;
    assert_eq!(listed.status, StatusCode::OK, "{}", listed.text);
    let page: v1::AuditEventPage = listed.decode();
    let actions: Vec<&str> = page
        .items
        .iter()
        .map(|event| event.action.as_str())
        .collect();
    assert_eq!(actions, ["signed_in", "signed_in"]);
    assert_eq!(page.items[0].detail.as_deref(), Some("password"));
    assert_eq!(page.items[0].ip.as_deref(), Some("203.0.113.7"));
    assert_eq!(page.items[0].user_agent.as_deref(), Some("Firefox"));
    assert!(page.items[0].user.is_none());
    assert!(page.next_cursor.is_some());
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn the_audit_log_needs_audit_read_and_records_the_admins_client(pool: PgPool) {
    let app = TestApp::new(pool);
    let user = app.register("alice@example.com").await;
    let admin = app.admin("admin@example.com").await;
    let id = app.user_id(&user).await;

    app.send(TestRequest::get("/api/v1/admin/audit"))
        .await
        .assert_problem(StatusCode::UNAUTHORIZED, "unauthenticated");
    app.send(TestRequest::get("/api/v1/admin/audit").session(&user))
        .await
        .assert_problem(StatusCode::FORBIDDEN, "forbidden");

    let disabled = app
        .send(
            TestRequest::post(&format!("/api/v1/admin/users/{id}/disable"))
                .session(&admin)
                .with_peer("198.51.100.4"),
        )
        .await;
    assert_eq!(disabled.status, StatusCode::OK, "{}", disabled.text);

    let listed = app
        .send(TestRequest::get(&format!("/api/v1/admin/audit?user={id}")).session(&admin))
        .await;
    assert_eq!(listed.status, StatusCode::OK, "{}", listed.text);
    let page: v1::AuditEventPage = listed.decode();
    let event = &page.items[0];
    assert_eq!(event.action, "account_disabled");
    assert!(event.by_other);
    assert_eq!(event.ip.as_deref(), Some("198.51.100.4"));
    assert_eq!(event.user.as_ref().unwrap().email, "alice@example.com");
    assert_eq!(event.actor.as_ref().unwrap().email, "admin@example.com");
    assert!(
        page.items
            .iter()
            .all(|event| event.user.as_ref().unwrap().id == id)
    );

    app.send(TestRequest::get("/api/v1/admin/audit?user=nope").session(&admin))
        .await
        .assert_field_error("user", "invalid_id");
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn admins_hold_audit_read(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.admin("admin@example.com").await;

    let me = app.me(&admin).await;

    assert!(
        me.permissions
            .contains(&i32::from(v1::Permission::AuditRead))
    );
}
