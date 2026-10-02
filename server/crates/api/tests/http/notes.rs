use axum::http::StatusCode;
use proto::v1;
use sqlx::PgPool;

use crate::support::{TestApp, TestRequest};

fn new_note(title: &str, body: Option<&str>) -> v1::CreateNoteRequest {
    v1::CreateNoteRequest {
        title: title.to_owned(),
        body: body.map(str::to_owned),
    }
}

fn changes(title: Option<&str>, body: Option<&str>) -> v1::UpdateNoteRequest {
    v1::UpdateNoteRequest {
        title: title.map(str::to_owned),
        body: body.map(str::to_owned),
    }
}

async fn create(app: &TestApp, token: &str, title: &str) -> v1::Note {
    let response = app
        .send(
            TestRequest::post("/api/v1/notes")
                .session(token)
                .proto(&new_note(title, Some("text"))),
        )
        .await;
    assert_eq!(response.status, StatusCode::CREATED, "{}", response.text);
    response.decode()
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn crud(pool: PgPool) {
    let app = TestApp::new(pool);
    let token = app.register("alice@example.com").await;

    let response = app
        .send(
            TestRequest::post("/api/v1/notes")
                .session(&token)
                .proto(&new_note("Groceries", None)),
        )
        .await;
    assert_eq!(response.status, StatusCode::CREATED);
    let note: v1::Note = response.decode();
    let id = note.id.clone();
    assert_eq!(
        response.header("location"),
        Some(format!("/api/v1/notes/{id}").as_str())
    );
    assert_eq!(note.body, "");
    assert_eq!(note.owner_id, app.user_id(&token).await);
    assert!(note.created_at.is_some());
    assert_eq!(note.created_at, note.updated_at);

    let fetched = app
        .send(TestRequest::get(&format!("/api/v1/notes/{id}")).session(&token))
        .await;
    assert_eq!(fetched.decode::<v1::Note>().title, "Groceries");

    let updated = app
        .send(
            TestRequest::patch(&format!("/api/v1/notes/{id}"))
                .session(&token)
                .proto(&changes(None, Some("milk"))),
        )
        .await;
    assert_eq!(updated.status, StatusCode::OK);
    let updated: v1::Note = updated.decode();
    assert_eq!(updated.title, "Groceries");
    assert_eq!(updated.body, "milk");

    let deleted = app
        .send(TestRequest::delete(&format!("/api/v1/notes/{id}")).session(&token))
        .await;
    assert_eq!(deleted.status, StatusCode::NO_CONTENT);
    app.send(TestRequest::get(&format!("/api/v1/notes/{id}")).session(&token))
        .await
        .assert_problem(StatusCode::NOT_FOUND, "not_found");
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn notes_need_a_session(pool: PgPool) {
    let app = TestApp::new(pool);
    for request in [
        TestRequest::get("/api/v1/notes"),
        TestRequest::post("/api/v1/notes").proto(&new_note("x", None)),
    ] {
        app.send(request)
            .await
            .assert_problem(StatusCode::UNAUTHORIZED, "unauthenticated");
    }
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn other_users_notes_are_not_found(pool: PgPool) {
    let app = TestApp::new(pool);
    let alice = app.register("alice@example.com").await;
    let bob = app.register("bob@example.com").await;
    let note = create(&app, &alice, "private").await;
    let path = format!("/api/v1/notes/{}", note.id);

    for request in [
        TestRequest::get(&path),
        TestRequest::patch(&path).proto(&changes(Some("mine now"), None)),
        TestRequest::delete(&path),
    ] {
        app.send(request.session(&bob))
            .await
            .assert_problem(StatusCode::NOT_FOUND, "not_found");
    }
    app.send(TestRequest::get("/api/v1/notes?scope=all").session(&bob))
        .await
        .assert_problem(StatusCode::FORBIDDEN, "forbidden");
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn admins_see_and_moderate_everyones_notes(pool: PgPool) {
    let app = TestApp::new(pool);
    let alice = app.register("alice@example.com").await;
    let admin = app.admin("admin@example.com").await;
    let note = create(&app, &alice, "alice's").await;
    create(&app, &admin, "admin's").await;

    let all = app
        .send(TestRequest::get("/api/v1/notes?scope=all").session(&admin))
        .await;
    assert_eq!(all.decode::<v1::NotePage>().items.len(), 2);
    let mine = app
        .send(TestRequest::get("/api/v1/notes").session(&admin))
        .await;
    assert_eq!(mine.decode::<v1::NotePage>().items.len(), 1);

    let moderated = app
        .send(
            TestRequest::patch(&format!("/api/v1/notes/{}", note.id))
                .session(&admin)
                .proto(&changes(Some("moderated"), None)),
        )
        .await;
    assert_eq!(moderated.status, StatusCode::OK);
    assert_eq!(moderated.decode::<v1::Note>().owner_id, note.owner_id);
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn paging_through_notes(pool: PgPool) {
    let app = TestApp::new(pool);
    let token = app.register("alice@example.com").await;
    for n in 1..=5 {
        create(&app, &token, &format!("note {n}")).await;
    }

    let mut titles = Vec::new();
    let mut uri = "/api/v1/notes?limit=2".to_owned();
    loop {
        let page = app.send(TestRequest::get(&uri).session(&token)).await;
        assert_eq!(page.status, StatusCode::OK, "{}", page.text);
        let page: v1::NotePage = page.decode();
        titles.extend(page.items.into_iter().map(|note| note.title));
        match page.next_cursor {
            Some(cursor) => uri = format!("/api/v1/notes?limit=2&after={cursor}"),
            None => break,
        }
    }

    assert_eq!(titles, ["note 5", "note 4", "note 3", "note 2", "note 1"]);
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn invalid_notes_and_queries(pool: PgPool) {
    let app = TestApp::new(pool);
    let token = app.register("alice@example.com").await;

    let invalid =
        app.send(TestRequest::post("/api/v1/notes").session(&token).proto(
            &v1::CreateNoteRequest {
                title: String::new(),
                body: Some("x".repeat(10_001)),
            },
        ))
        .await;
    invalid.assert_problem(StatusCode::UNPROCESSABLE_ENTITY, "validation_failed");
    assert_eq!(invalid.body["errors"].as_array().unwrap().len(), 2);

    app.send(TestRequest::get("/api/v1/notes?after=garbage").session(&token))
        .await
        .assert_problem(StatusCode::UNPROCESSABLE_ENTITY, "validation_failed");
    app.send(TestRequest::get("/api/v1/notes?scope=everyone").session(&token))
        .await
        .assert_problem(StatusCode::BAD_REQUEST, "invalid_query");
    app.send(TestRequest::get("/api/v1/notes/not-a-uuid").session(&token))
        .await
        .assert_problem(StatusCode::NOT_FOUND, "not_found");
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn duplicating_a_note(pool: PgPool) {
    let app = TestApp::new(pool);
    let alice = app.register("alice@example.com").await;
    let bob = app.register("bob@example.com").await;
    let created = app
        .send(
            TestRequest::post("/api/v1/notes")
                .session(&alice)
                .proto(&new_note("Recipe", Some("flour"))),
        )
        .await;
    let created: v1::Note = created.decode();
    let id = created.id.clone();

    let copy = app
        .send(TestRequest::post(&format!("/api/v1/notes/{id}/duplicate")).session(&alice))
        .await;
    assert_eq!(copy.status, StatusCode::CREATED, "{}", copy.text);
    let location = copy.header("location").unwrap().to_owned();
    let copy: v1::Note = copy.decode();
    assert_ne!(copy.id, created.id);
    assert_eq!(copy.title, "Recipe");
    assert_eq!(copy.body, "flour");
    assert!(location.ends_with(&copy.id));

    app.send(TestRequest::post(&format!("/api/v1/notes/{id}/duplicate")).session(&bob))
        .await
        .assert_problem(StatusCode::NOT_FOUND, "not_found");
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn only_large_responses_are_compressed(pool: PgPool) {
    let app = TestApp::new(pool);
    let token = app.register("alice@example.com").await;
    let small = create(&app, &token, "Small").await;
    let large = app
        .send(
            TestRequest::post("/api/v1/notes")
                .session(&token)
                .proto(&new_note("Large", Some(&"lorem ipsum ".repeat(200)))),
        )
        .await
        .decode::<v1::Note>();

    let fetch = |id: &str| {
        TestRequest::get(&format!("/api/v1/notes/{id}"))
            .session(&token)
            .header("accept-encoding", "br")
    };
    let response = app.send(fetch(&small.id)).await;
    assert_eq!(response.status, StatusCode::OK);
    assert!(response.header("content-encoding").is_none());
    let response = app.send(fetch(&large.id)).await;
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.header("content-encoding"), Some("br"));
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn versions_travel_as_etag_and_if_match(pool: PgPool) {
    let app = TestApp::new(pool);
    let token = app.register("alice@example.com").await;
    let note = create(&app, &token, "Draft").await;
    assert_eq!(note.version, 1);
    let uri = format!("/api/v1/notes/{}", note.id);

    let fetched = app.send(TestRequest::get(&uri).session(&token)).await;
    assert_eq!(fetched.header("etag"), Some("\"1\""));

    let updated = app
        .send(
            TestRequest::patch(&uri)
                .session(&token)
                .header("if-match", "\"1\"")
                .proto(&changes(Some("Final"), None)),
        )
        .await;
    assert_eq!(updated.status, StatusCode::OK, "{}", updated.text);
    assert_eq!(updated.header("etag"), Some("\"2\""));
    assert_eq!(updated.decode::<v1::Note>().version, 2);

    // Someone still holding version 1, or sending a tag this server never issues.
    for tag in ["\"1\"", "W/\"2\"", "2", "\"2\", \"3\""] {
        app.send(
            TestRequest::patch(&uri)
                .session(&token)
                .header("if-match", tag)
                .proto(&changes(Some("Lost"), None)),
        )
        .await
        .assert_problem(StatusCode::PRECONDITION_FAILED, "stale");
    }
    app.send(
        TestRequest::delete(&uri)
            .session(&token)
            .header("if-match", "\"1\""),
    )
    .await
    .assert_problem(StatusCode::PRECONDITION_FAILED, "stale");

    let unconditional = app
        .send(
            TestRequest::patch(&uri)
                .session(&token)
                .header("if-match", "*")
                .proto(&changes(None, Some("any"))),
        )
        .await;
    assert_eq!(unconditional.decode::<v1::Note>().title, "Final");

    let deleted = app
        .send(
            TestRequest::delete(&uri)
                .session(&token)
                .header("if-match", "\"3\""),
        )
        .await;
    assert_eq!(deleted.status, StatusCode::NO_CONTENT, "{}", deleted.text);
}
