use axum::http::StatusCode;
use proto::v1;
use sqlx::PgPool;

use crate::support::{PASSWORD, TestApp, TestRequest, user};

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn admin_endpoints_are_401_anonymous_and_403_without_permission(pool: PgPool) {
    let app = TestApp::new(pool);
    let user = app.register("alice@example.com").await;
    let admin = app.admin("admin@example.com").await;
    let id = app.user_id(&user).await;

    let requests = || {
        [
            TestRequest::get("/api/v1/admin/users"),
            TestRequest::get(&format!("/api/v1/admin/users/{id}")),
            TestRequest::get("/api/v1/admin/roles"),
            TestRequest::put(&format!("/api/v1/admin/users/{id}/roles/admin")),
            TestRequest::delete(&format!("/api/v1/admin/users/{id}/roles/user")),
            TestRequest::post(&format!("/api/v1/admin/users/{id}/disable")),
            TestRequest::post(&format!("/api/v1/admin/users/{id}/enable")),
            TestRequest::get(&format!("/api/v1/admin/users/{id}/sessions")),
            TestRequest::delete(&format!("/api/v1/admin/users/{id}/sessions")),
            TestRequest::delete(&format!(
                "/api/v1/admin/users/{id}/sessions/{}",
                uuid::Uuid::nil()
            )),
        ]
    };
    for request in requests() {
        app.send(request)
            .await
            .assert_problem(StatusCode::UNAUTHORIZED, "unauthenticated");
    }
    for request in requests() {
        app.send(request.session(&user))
            .await
            .assert_problem(StatusCode::FORBIDDEN, "forbidden");
    }

    let listed = app
        .send(TestRequest::get("/api/v1/admin/users").session(&admin))
        .await;
    assert_eq!(listed.status, StatusCode::OK, "{}", listed.text);
    assert_eq!(listed.decode::<v1::UserPage>().items.len(), 2);
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn listing_users_pages_and_searches(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.admin("admin@example.com").await;
    for name in ["alice", "bob", "carol"] {
        app.register(&format!("{name}@example.com")).await;
    }

    let first = app
        .send(TestRequest::get("/api/v1/admin/users?limit=2").session(&admin))
        .await;
    let first: v1::UserPage = first.decode();
    assert_eq!(first.items[0].email, "carol@example.com");
    let cursor = first.next_cursor.unwrap();

    let second = app
        .send(
            TestRequest::get(&format!("/api/v1/admin/users?limit=2&after={cursor}"))
                .session(&admin),
        )
        .await;
    let second: v1::UserPage = second.decode();
    assert_eq!(second.items[1].email, "admin@example.com");
    assert_eq!(second.items[1].roles, ["admin", "user"]);
    assert!(second.next_cursor.is_none());

    let search = app
        .send(TestRequest::get("/api/v1/admin/users?search=BOB").session(&admin))
        .await;
    assert_eq!(search.decode::<v1::UserPage>().items.len(), 1);
    app.send(TestRequest::get("/api/v1/admin/users?search=bo").session(&admin))
        .await
        .assert_problem(StatusCode::UNPROCESSABLE_ENTITY, "validation_failed");

    app.send(TestRequest::get("/api/v1/admin/users?limit=500").session(&admin))
        .await
        .assert_problem(StatusCode::UNPROCESSABLE_ENTITY, "validation_failed");

    let roles = app
        .send(TestRequest::get("/api/v1/admin/roles").session(&admin))
        .await;
    let roles = roles.decode::<v1::RoleList>().roles;
    assert_eq!(roles[1].name, "user");
    assert_eq!(
        roles[1].permissions().collect::<Vec<_>>(),
        crate::support::default_permissions()
    );
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn a_role_change_rotates_the_users_session_on_their_next_request(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.admin("admin@example.com").await;
    let alice = app.register("alice@example.com").await;
    let id = app.user_id(&alice).await;

    let granted = app
        .send(TestRequest::put(&format!("/api/v1/admin/users/{id}/roles/admin")).session(&admin))
        .await;
    assert_eq!(granted.status, StatusCode::OK, "{}", granted.text);
    assert_eq!(granted.decode::<v1::User>().roles, ["admin", "user"]);

    let next = app
        .send(TestRequest::get("/api/v1/me").session(&alice))
        .await;
    assert_eq!(next.status, StatusCode::OK);
    assert!(
        next.decode::<v1::Me>()
            .permissions()
            .any(|permission| permission == v1::Permission::UsersManage)
    );
    let rotated = next.session_token().expect("the session was not rotated");
    assert_ne!(rotated, alice);

    let old = app
        .send(TestRequest::get("/api/v1/me").session(&alice))
        .await;
    assert_eq!(old.status, StatusCode::UNAUTHORIZED);
    let again = app
        .send(TestRequest::get("/api/v1/me").session(&rotated))
        .await;
    assert!(again.session_cookie().is_none());

    app.send(TestRequest::put(&format!("/api/v1/admin/users/{id}/roles/ghost")).session(&admin))
        .await
        .assert_problem(StatusCode::NOT_FOUND, "not_found");
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn disabling_a_user_signs_them_out_and_blocks_sign_in(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.admin("admin@example.com").await;
    let alice = app.register("alice@example.com").await;
    let id = app.user_id(&alice).await;

    let disabled = app
        .send(TestRequest::post(&format!("/api/v1/admin/users/{id}/disable")).session(&admin))
        .await;
    assert_eq!(disabled.status, StatusCode::OK, "{}", disabled.text);
    assert!(disabled.decode::<v1::User>().disabled);

    let me = app
        .send(TestRequest::get("/api/v1/me").session(&alice))
        .await;
    assert_eq!(me.status, StatusCode::UNAUTHORIZED);
    app.login("alice@example.com", PASSWORD)
        .await
        .assert_problem(StatusCode::FORBIDDEN, "account_disabled");

    app.send(TestRequest::post(&format!("/api/v1/admin/users/{id}/enable")).session(&admin))
        .await;
    assert_eq!(
        app.login("alice@example.com", PASSWORD).await.status,
        StatusCode::OK
    );
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn admins_cannot_lock_everyone_out(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.admin("admin@example.com").await;
    let id = app.user_id(&admin).await;

    app.send(TestRequest::post(&format!("/api/v1/admin/users/{id}/disable")).session(&admin))
        .await
        .assert_problem(StatusCode::CONFLICT, "cannot_disable_self");
    app.send(TestRequest::delete(&format!("/api/v1/admin/users/{id}/roles/admin")).session(&admin))
        .await
        .assert_problem(StatusCode::CONFLICT, "last_admin");
    app.send(
        TestRequest::delete("/api/v1/me")
            .session(&admin)
            .proto(&v1::DeleteAccountRequest {
                password: Some(PASSWORD.to_owned()),
            }),
    )
    .await
    .assert_problem(StatusCode::CONFLICT, "last_admin");

    let me = app.me(&admin).await;
    assert_eq!(user(&me).roles, ["admin", "user"]);
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn unknown_users_are_404(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.admin("admin@example.com").await;
    let id = uuid::Uuid::now_v7();

    app.send(TestRequest::get(&format!("/api/v1/admin/users/{id}")).session(&admin))
        .await
        .assert_problem(StatusCode::NOT_FOUND, "not_found");
    app.send(TestRequest::put(&format!("/api/v1/admin/users/{id}/roles/admin")).session(&admin))
        .await
        .assert_problem(StatusCode::NOT_FOUND, "not_found");
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn admins_list_and_end_a_users_sessions(pool: PgPool) {
    let app = TestApp::new(pool);
    let admin = app.admin("admin@example.com").await;
    let alice = app.register("alice@example.com").await;
    let second = app
        .login("alice@example.com", PASSWORD)
        .await
        .session_token()
        .unwrap();
    let id = app.user_id(&alice).await;
    let sessions_uri = format!("/api/v1/admin/users/{id}/sessions");

    let listed = app
        .send(TestRequest::get(&sessions_uri).session(&admin))
        .await;
    assert_eq!(listed.status, StatusCode::OK, "{}", listed.text);
    let sessions = listed.decode::<v1::SessionList>().sessions;
    assert_eq!(sessions.len(), 2);
    assert!(sessions.iter().all(|session| !session.current));

    let revoked = app
        .send(TestRequest::delete(&format!("{sessions_uri}/{}", sessions[0].id)).session(&admin))
        .await;
    assert_eq!(revoked.status, StatusCode::NO_CONTENT, "{}", revoked.text);
    let remaining = app
        .send(TestRequest::get(&sessions_uri).session(&admin))
        .await
        .decode::<v1::SessionList>()
        .sessions;
    assert_eq!(remaining.len(), 1);

    let signed_out = app
        .send(TestRequest::delete(&sessions_uri).session(&admin))
        .await;
    assert_eq!(
        signed_out.status,
        StatusCode::NO_CONTENT,
        "{}",
        signed_out.text
    );
    for token in [&alice, &second] {
        let me = app
            .send(TestRequest::get("/api/v1/me").session(token))
            .await;
        assert_eq!(me.status, StatusCode::UNAUTHORIZED);
    }
    // Signed out, not disabled.
    assert_eq!(
        app.login("alice@example.com", PASSWORD).await.status,
        StatusCode::OK
    );
}
