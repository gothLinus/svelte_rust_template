use axum::http::StatusCode;
use proto::v1;
use sqlx::PgPool;

use crate::support::{Options, PASSWORD, TestApp, TestRequest, user};

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn updating_the_profile(pool: PgPool) {
    let app = TestApp::new(pool);
    let token = app.register("alice@example.com").await;

    let response =
        app.send(TestRequest::patch("/api/v1/me").session(&token).proto(
            &v1::UpdateProfileRequest {
                username: "  Alice.Liddell ".to_owned(),
            },
        ))
        .await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.text);
    assert_eq!(user(&response.decode()).username, "alice.liddell");

    let invalid = app
        .send(
            TestRequest::patch("/api/v1/me")
                .session(&token)
                .proto(&v1::UpdateProfileRequest::default()),
        )
        .await;
    invalid.assert_problem(StatusCode::UNPROCESSABLE_ENTITY, "validation_failed");
    assert_eq!(invalid.body["errors"][0]["field"], "username");
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn changing_the_password_goes_through_a_mailed_link(pool: PgPool) {
    let app = TestApp::new(pool);
    let current = app.register("alice@example.com").await;

    let response = app
        .send(TestRequest::post("/api/v1/me/password").session(&current))
        .await;
    assert_eq!(response.status, StatusCode::ACCEPTED, "{}", response.text);
    let old = app
        .send(TestRequest::put("/api/v1/me/password").session(&current))
        .await;
    assert_eq!(old.status, StatusCode::METHOD_NOT_ALLOWED);

    let reset = app
        .send(
            TestRequest::post("/api/v1/auth/reset-password").proto(&v1::ResetPasswordRequest {
                token: app.mailed_token("alice@example.com"),
                password: "a new password".to_owned(),
            }),
        )
        .await;
    assert_eq!(reset.status, StatusCode::NO_CONTENT, "{}", reset.text);

    let me = app
        .send(TestRequest::get("/api/v1/me").session(&current))
        .await;
    assert_eq!(me.status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        app.login("alice@example.com", "a new password")
            .await
            .status,
        StatusCode::OK
    );
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn listing_and_revoking_sessions(pool: PgPool) {
    let app = TestApp::new(pool);
    let current = app.register("alice@example.com").await;
    let other = app
        .login("alice@example.com", PASSWORD)
        .await
        .session_token()
        .unwrap();

    let sessions = app
        .send(TestRequest::get("/api/v1/me/sessions").session(&current))
        .await;
    assert_eq!(sessions.status, StatusCode::OK);
    let list = sessions.decode::<v1::SessionList>().sessions;
    assert_eq!(list.len(), 2);
    let current_entry = list.iter().find(|s| s.current).unwrap();
    let other_entry = list.iter().find(|s| !s.current).unwrap();
    assert_eq!(current_entry.ip.as_deref(), Some("192.0.2.1"));
    let seconds = |at: Option<&proto::Timestamp>| at.unwrap().seconds;
    assert!(
        seconds(current_entry.expires_at.as_ref()) > seconds(current_entry.created_at.as_ref())
    );

    let revoke = app
        .send(
            TestRequest::delete(&format!("/api/v1/me/sessions/{}", other_entry.id))
                .session(&current),
        )
        .await;
    assert_eq!(revoke.status, StatusCode::NO_CONTENT);
    assert!(revoke.session_cookie().is_none());
    let revoked = app
        .send(TestRequest::get("/api/v1/me").session(&other))
        .await;
    assert_eq!(revoked.status, StatusCode::UNAUTHORIZED);

    let own = app
        .send(
            TestRequest::delete(&format!("/api/v1/me/sessions/{}", current_entry.id))
                .session(&current),
        )
        .await;
    assert_eq!(own.status, StatusCode::NO_CONTENT);
    assert!(own.clears_session());
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn other_users_sessions_cannot_be_revoked(pool: PgPool) {
    let app = TestApp::new(pool);
    let alice = app.register("alice@example.com").await;
    let bob = app.register("bob@example.com").await;
    let bobs = app
        .send(TestRequest::get("/api/v1/me/sessions").session(&bob))
        .await;
    let id = bobs.decode::<v1::SessionList>().sessions[0].id.clone();

    app.send(TestRequest::delete(&format!("/api/v1/me/sessions/{id}")).session(&alice))
        .await
        .assert_problem(StatusCode::NOT_FOUND, "not_found");
    app.me(&bob).await;
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn session_ips_come_from_the_proxy_only_when_trusted(pool: PgPool) {
    let app = TestApp::with(
        pool,
        Options {
            trust_proxy: true,
            ..Options::default()
        },
    );
    let token = app
        .send(
            TestRequest::post("/api/v1/auth/register")
                .header("x-forwarded-for", "203.0.113.9, 198.51.100.7")
                .proto(&v1::RegisterRequest {
                    email: "alice@example.com".to_owned(),
                    username: "alice".to_owned(),
                    password: PASSWORD.to_owned(),
                }),
        )
        .await
        .session_token()
        .unwrap();

    let sessions = app
        .send(TestRequest::get("/api/v1/me/sessions").session(&token))
        .await;
    assert_eq!(
        sessions.decode::<v1::SessionList>().sessions[0]
            .ip
            .as_deref(),
        Some("198.51.100.7")
    );
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn deleting_the_account(pool: PgPool) {
    let app = TestApp::new(pool);
    let token = app.register("alice@example.com").await;

    app.send(
        TestRequest::delete("/api/v1/me")
            .session(&token)
            .proto(&v1::DeleteAccountRequest {
                password: Some("wrong password".to_owned()),
            }),
    )
    .await
    .assert_problem(StatusCode::UNPROCESSABLE_ENTITY, "validation_failed");

    let response =
        app.send(TestRequest::delete("/api/v1/me").session(&token).proto(
            &v1::DeleteAccountRequest {
                password: Some(PASSWORD.to_owned()),
            },
        ))
        .await;
    assert_eq!(response.status, StatusCode::NO_CONTENT);
    assert!(response.clears_session());
    app.login("alice@example.com", PASSWORD)
        .await
        .assert_problem(StatusCode::UNAUTHORIZED, "invalid_credentials");
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn the_export_holds_the_users_data_and_no_secrets(pool: PgPool) {
    let app = TestApp::new(pool);
    let token = app.register_verified("alice@example.com").await;
    let note = app
        .send(TestRequest::post("/api/v1/notes").session(&token).proto(
            &proto::v1::CreateNoteRequest {
                title: "Shopping".to_owned(),
                body: Some("Milk".to_owned()),
            },
        ))
        .await;
    assert_eq!(note.status, StatusCode::CREATED, "{}", note.text);

    let export = app
        .send(TestRequest::get("/api/v1/me/export").session(&token))
        .await;
    assert_eq!(export.status, StatusCode::OK, "{}", export.text);
    assert!(
        export
            .header("content-disposition")
            .is_some_and(|value| value.starts_with("attachment"))
    );
    let body = &export.body;
    assert_eq!(body["account"]["email"], "alice@example.com");
    assert_eq!(body["account"]["roles"], serde_json::json!(["user"]));
    assert_eq!(body["sessions"].as_array().unwrap().len(), 1);
    assert_eq!(body["notes"][0]["title"], "Shopping");
    assert_eq!(body["notes"][0]["body"], "Milk");
    for secret in ["argon2", "token_hash", "password_hash", "sealed"] {
        assert!(!export.text.contains(secret), "{secret} in {}", export.text);
    }
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn every_table_about_users_is_exported_or_left_out_on_purpose(pool: PgPool) {
    use application::export::{EXPORTED_TABLES, Exportable, NOT_EXPORTED_TABLES};

    assert!(EXPORTED_TABLES.contains(&<domain::note::Note as Exportable>::TABLE));

    let referencing: Vec<String> = sqlx::query_scalar(
        "select distinct tc.table_name::text
         from information_schema.table_constraints tc
         join information_schema.constraint_column_usage ccu
           on tc.constraint_name = ccu.constraint_name
         where tc.constraint_type = 'FOREIGN KEY' and ccu.table_name = 'users'
         order by 1",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    for table in &referencing {
        let exported = EXPORTED_TABLES.contains(&table.as_str());
        let left_out = NOT_EXPORTED_TABLES.iter().any(|(name, _)| name == table);
        assert!(
            exported ^ left_out,
            "`{table}` references users: add it to the export (application/src/export.rs, \
             and an `Exportable` impl for a resource) or to NOT_EXPORTED_TABLES with a reason"
        );
    }
    for table in EXPORTED_TABLES
        .iter()
        .chain(NOT_EXPORTED_TABLES.iter().map(|(t, _)| t))
    {
        assert!(
            *table == "users" || referencing.iter().any(|name| name == table),
            "`{table}` is listed but does not reference users"
        );
    }
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn the_export_needs_a_recent_sign_in(pool: PgPool) {
    let app = TestApp::new(pool);
    let token = app.register_verified("alice@example.com").await;
    app.clock
        .advance(domain::session::REAUTH_WINDOW + time::Duration::seconds(1));

    app.send(TestRequest::get("/api/v1/me/export").session(&token))
        .await
        .assert_problem(StatusCode::FORBIDDEN, "reauth_required");
}
