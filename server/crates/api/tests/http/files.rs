use std::time::Duration;

use api::rate_limit::{Rate, Rates};
use axum::{
    body::{Body, Bytes},
    http::StatusCode,
};
use domain::object_store::ObjectKey;
use proto::v1;
use sqlx::PgPool;

use crate::support::{Options, TestApp, TestRequest};

fn upload_request(token: &str, name: &str, content_type: &str, contents: &[u8]) -> TestRequest {
    TestRequest::post(&format!("/api/v1/files?name={}", urlencode(name)))
        .session(token)
        .body(content_type, contents.to_vec())
        .header("content-length", &contents.len().to_string())
}

fn urlencode(name: &str) -> String {
    url::form_urlencoded::byte_serialize(name.as_bytes()).collect()
}

async fn upload(app: &TestApp, token: &str, name: &str, contents: &[u8]) -> v1::StoredFile {
    let response = app
        .send(upload_request(token, name, "text/plain", contents))
        .await;
    assert_eq!(response.status, StatusCode::CREATED, "{}", response.text);
    response.decode()
}

async fn object_key(app: &TestApp, file: &v1::StoredFile) -> ObjectKey {
    let key: String = sqlx::query_scalar("select object_key from files where id = $1::uuid")
        .bind(&file.id)
        .fetch_one(&app.pool)
        .await
        .unwrap();
    ObjectKey::parse(&key).unwrap()
}

async fn queued(app: &TestApp) -> Vec<String> {
    sqlx::query_scalar("select object_key from object_deletions order by object_key")
        .fetch_all(&app.pool)
        .await
        .unwrap()
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn upload_list_download_rename_and_delete(pool: PgPool) {
    let app = TestApp::new(pool);
    let token = app.register("alice@example.com").await;

    let response = app
        .send(upload_request(
            &token,
            "Report Q3.pdf",
            "application/pdf",
            b"%PDF-1.7 contents",
        ))
        .await;
    assert_eq!(response.status, StatusCode::CREATED, "{}", response.text);
    let file: v1::StoredFile = response.decode();
    assert_eq!(
        response.header("location"),
        Some(format!("/api/v1/files/{}", file.id).as_str())
    );
    assert_eq!(file.name, "Report Q3.pdf");
    assert_eq!(file.content_type, "application/pdf");
    assert_eq!(file.size, 17);
    assert_eq!(file.owner_id, app.user_id(&token).await);
    let key = object_key(&app, &file).await;
    assert_eq!(
        app.objects.object(&key).unwrap().1,
        &b"%PDF-1.7 contents"[..]
    );
    assert!(queued(&app).await.is_empty());

    let page: v1::StoredFilePage = app
        .send(TestRequest::get("/api/v1/files").session(&token))
        .await
        .decode();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].id, file.id);

    let shown: v1::StoredFile = app
        .send(TestRequest::get(&format!("/api/v1/files/{}", file.id)).session(&token))
        .await
        .decode();
    assert_eq!(shown, file);

    let download = app
        .send(
            TestRequest::get(&format!("/api/v1/files/{}/content", file.id))
                .session(&token)
                .header("accept-encoding", "gzip, br"),
        )
        .await;
    assert_eq!(download.status, StatusCode::OK);
    assert_eq!(download.bytes, &b"%PDF-1.7 contents"[..]);
    assert_eq!(download.header("content-type"), Some("application/pdf"));
    assert_eq!(download.header("content-length"), Some("17"));
    assert_eq!(
        download.header("content-disposition"),
        Some("attachment; filename=\"Report Q3.pdf\"; filename*=UTF-8''Report%20Q3.pdf")
    );
    // Sent as it is stored, never compressed on the way, and never run by the browser.
    assert_eq!(download.header("content-encoding"), None);
    assert_eq!(download.header("x-content-type-options"), Some("nosniff"));
    assert!(
        download
            .header("content-security-policy")
            .unwrap()
            .starts_with("default-src 'none'")
    );
    assert_eq!(download.header("cache-control"), Some("no-store"));

    let renamed: v1::StoredFile = app
        .send(
            TestRequest::patch(&format!("/api/v1/files/{}", file.id))
                .session(&token)
                .proto(&v1::UpdateFileRequest {
                    name: Some("Q3.pdf".to_owned()),
                }),
        )
        .await
        .decode();
    assert_eq!(renamed.name, "Q3.pdf");

    let deleted = app
        .send(TestRequest::delete(&format!("/api/v1/files/{}", file.id)).session(&token))
        .await;
    assert_eq!(deleted.status, StatusCode::NO_CONTENT);
    assert!(app.objects.object(&key).is_none());
    assert!(queued(&app).await.is_empty());
    app.send(TestRequest::get(&format!("/api/v1/files/{}", file.id)).session(&token))
        .await
        .assert_problem(StatusCode::NOT_FOUND, "not_found");
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn files_larger_than_the_body_limit_go_through_uncompressed(pool: PgPool) {
    let app = TestApp::new(pool);
    let token = app.register("alice@example.com").await;
    // Over the default `MAX_BODY_BYTES` (64 KiB) that caps every other request, and as compressible
    // as text gets.
    let contents = b"lorem ipsum ".repeat(10_000);

    let file = upload(&app, &token, "large.txt", &contents).await;
    let download = app
        .send(
            TestRequest::get(&format!("/api/v1/files/{}/content", file.id))
                .session(&token)
                .header("accept-encoding", "gzip, br"),
        )
        .await;

    assert_eq!(file.size, 120_000);
    assert_eq!(download.status, StatusCode::OK);
    assert_eq!(download.header("content-length"), Some("120000"));
    assert_eq!(download.header("content-encoding"), None);
    assert_eq!(download.bytes, contents);
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn downloads_name_any_file_safely(pool: PgPool) {
    let app = TestApp::new(pool);
    let token = app.register("alice@example.com").await;

    let file = upload(&app, &token, "Grüße \"final\".txt", b"hallo").await;

    let download = app
        .send(TestRequest::get(&format!("/api/v1/files/{}/content", file.id)).session(&token))
        .await;
    assert_eq!(
        download.header("content-disposition"),
        Some(
            "attachment; filename=\"Gr__e _final_.txt\"; \
             filename*=UTF-8''Gr%C3%BC%C3%9Fe%20%22final%22.txt"
        )
    );
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn an_upload_needs_a_length(pool: PgPool) {
    let app = TestApp::new(pool);
    let token = app.register("alice@example.com").await;

    let response = app
        .send(
            TestRequest::post("/api/v1/files?name=a.txt")
                .session(&token)
                .body("text/plain", "abc"),
        )
        .await;
    response.assert_problem(StatusCode::LENGTH_REQUIRED, "length_required");
    assert_eq!(response.body["title"], app.text("http-status-411"));
    assert_eq!(response.body["detail"], app.text("http-length-required"));

    app.send(
        TestRequest::post("/api/v1/files?name=a.txt")
            .session(&token)
            .body("text/plain", "abc")
            .header("content-length", "three"),
    )
    .await
    .assert_problem(StatusCode::BAD_REQUEST, "invalid_request");
    assert!(app.objects.keys().is_empty());
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn an_upload_is_checked_before_its_body_is_read(pool: PgPool) {
    let app = TestApp::new(pool);
    let token = app.register("alice@example.com").await;

    let response = app
        .send(
            TestRequest::post("/api/v1/files?name=huge.iso")
                .session(&token)
                .body("application/octet-stream", Body::empty())
                .header(
                    "content-length",
                    &(domain::file::MAX_FILE_SIZE + 1).to_string(),
                ),
        )
        .await;
    response.assert_field_error("file", "too_large");

    app.send(upload_request(&token, "", "text/plain", b"abc"))
        .await
        .assert_field_error("name", "required");
    app.send(upload_request(&token, "a.txt", "text", b"abc"))
        .await
        .assert_field_error("contentType", "invalid_content_type");
    assert!(app.objects.keys().is_empty());
    assert!(queued(&app).await.is_empty());
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn a_body_shorter_than_announced_stores_nothing(pool: PgPool) {
    let app = TestApp::new(pool);
    let token = app.register("alice@example.com").await;

    let response = app
        .send(
            TestRequest::post("/api/v1/files?name=a.txt")
                .session(&token)
                .body("text/plain", "abc")
                .header("content-length", "10"),
        )
        .await;

    response.assert_problem(StatusCode::BAD_REQUEST, "incomplete_upload");
    assert_eq!(response.body["detail"], app.text("error-incomplete-upload"));
    let page: v1::StoredFilePage = app
        .send(TestRequest::get("/api/v1/files").session(&token))
        .await
        .decode();
    assert!(page.items.is_empty());
    assert_eq!(queued(&app).await.len(), 1);
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn a_store_that_is_down_is_an_internal_error(pool: PgPool) {
    let app = TestApp::new(pool);
    let token = app.register("alice@example.com").await;
    let file = upload(&app, &token, "a.txt", b"abc").await;
    app.objects.set_down(true);

    app.send(upload_request(&token, "b.txt", "text/plain", b"def"))
        .await
        .assert_problem(StatusCode::INTERNAL_SERVER_ERROR, "internal_error");
    app.send(TestRequest::get(&format!("/api/v1/files/{}/content", file.id)).session(&token))
        .await
        .assert_problem(StatusCode::INTERNAL_SERVER_ERROR, "internal_error");
    let key = object_key(&app, &file).await;
    let deleted = app
        .send(TestRequest::delete(&format!("/api/v1/files/{}", file.id)).session(&token))
        .await;
    assert_eq!(deleted.status, StatusCode::NO_CONTENT);
    let queued = queued(&app).await;
    assert_eq!(queued.len(), 2);
    assert!(queued.contains(&key.to_string()));
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn files_need_a_session_and_the_csrf_header(pool: PgPool) {
    let app = TestApp::new(pool);
    let token = app.register("alice@example.com").await;

    app.send(TestRequest::get("/api/v1/files"))
        .await
        .assert_problem(StatusCode::UNAUTHORIZED, "unauthenticated");
    let response = app
        .send(upload_request(&token, "a.txt", "text/plain", b"abc").without_csrf_headers())
        .await;
    response.assert_problem(StatusCode::FORBIDDEN, "csrf_rejected");
    assert!(app.objects.keys().is_empty());
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn other_users_files_are_not_found(pool: PgPool) {
    let app = TestApp::new(pool);
    let alice = app.register("alice@example.com").await;
    let bob = app.register("bob@example.com").await;
    let file = upload(&app, &alice, "private.txt", b"secret").await;

    for request in [
        TestRequest::get(&format!("/api/v1/files/{}", file.id)),
        TestRequest::get(&format!("/api/v1/files/{}/content", file.id)),
        TestRequest::delete(&format!("/api/v1/files/{}", file.id)),
        TestRequest::patch(&format!("/api/v1/files/{}", file.id)).proto(&v1::UpdateFileRequest {
            name: Some("mine.txt".to_owned()),
        }),
        TestRequest::get("/api/v1/files/not-a-uuid/content"),
    ] {
        app.send(request.session(&bob))
            .await
            .assert_problem(StatusCode::NOT_FOUND, "not_found");
    }
    app.send(TestRequest::get("/api/v1/files?scope=all").session(&bob))
        .await
        .assert_problem(StatusCode::FORBIDDEN, "forbidden");

    let admin = app.admin("admin@example.com").await;
    let everyone: v1::StoredFilePage = app
        .send(TestRequest::get("/api/v1/files?scope=all").session(&admin))
        .await
        .decode();
    assert_eq!(everyone.items.len(), 1);
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn usage_and_the_quota(pool: PgPool) {
    let app = TestApp::with(
        pool,
        Options {
            file_quota: Some(8),
            ..Options::default()
        },
    );
    let token = app.register("alice@example.com").await;
    upload(&app, &token, "five.txt", b"12345").await;

    let usage: v1::FileUsage = app
        .send(TestRequest::get("/api/v1/files/usage").session(&token))
        .await
        .decode();
    assert_eq!(usage.used_bytes, 5);
    assert_eq!(usage.quota_bytes, Some(8));

    let response = app
        .send(upload_request(&token, "four.txt", "text/plain", b"1234"))
        .await;
    response.assert_problem(StatusCode::CONFLICT, "file_quota_exceeded");
    assert_eq!(
        response.body["detail"],
        app.text("conflict-file-quota-exceeded")
    );
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn uploads_have_their_own_rate_limit(pool: PgPool) {
    let generous = Rate::new(
        std::num::NonZeroU32::new(1000).unwrap(),
        Duration::from_mins(1),
    );
    let two = Rate::new(
        std::num::NonZeroU32::new(2).unwrap(),
        Duration::from_hours(1),
    );
    let app = TestApp::with(
        pool,
        Options {
            rates: Some(Rates {
                upload_per_ip: generous,
                upload_per_account: two,
                ..Rates::default()
            }),
            ..Options::default()
        },
    );
    let token = app.register("alice@example.com").await;

    upload(&app, &token, "1.txt", b"1").await;
    upload(&app, &token, "2.txt", b"2").await;
    let response = app
        .send(upload_request(&token, "3.txt", "text/plain", b"3"))
        .await;

    response.assert_problem(StatusCode::TOO_MANY_REQUESTS, "rate_limited");
    assert!(response.header("retry-after").is_some());
    let page = app
        .send(TestRequest::get("/api/v1/files").session(&token))
        .await;
    assert_eq!(page.status, StatusCode::OK);
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn uploads_may_take_longer_than_other_requests(pool: PgPool) {
    let app = TestApp::with(
        pool,
        Options {
            request_timeout: Duration::from_millis(200),
            ..Options::default()
        },
    );
    let token = app.register("alice@example.com").await;
    let slow = || {
        Body::from_stream(futures_util::stream::once(async {
            tokio::time::sleep(Duration::from_millis(400)).await;
            Ok::<_, std::io::Error>(Bytes::from_static(b"slow"))
        }))
    };

    let upload = app
        .send(
            TestRequest::post("/api/v1/files?name=slow.txt")
                .session(&token)
                .body("text/plain", slow())
                .header("content-length", "4"),
        )
        .await;
    assert_eq!(upload.status, StatusCode::CREATED, "{}", upload.text);

    let other = app
        .send(
            TestRequest::patch("/api/v1/me")
                .session(&token)
                .body("application/x-protobuf", slow()),
        )
        .await;
    other.assert_problem(StatusCode::SERVICE_UNAVAILABLE, "timeout");
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn deleting_the_account_queues_its_files(pool: PgPool) {
    let app = TestApp::new(pool);
    let token = app.register("alice@example.com").await;
    let file = upload(&app, &token, "a.txt", b"abc").await;
    let key = object_key(&app, &file).await;

    let response =
        app.send(TestRequest::delete("/api/v1/me").session(&token).proto(
            &v1::DeleteAccountRequest {
                password: Some(crate::support::PASSWORD.to_owned()),
            },
        ))
        .await;
    assert_eq!(response.status, StatusCode::NO_CONTENT, "{}", response.text);

    assert_eq!(queued(&app).await, [key.to_string()]);
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn the_export_lists_files(pool: PgPool) {
    let app = TestApp::new(pool);
    let token = app.register_verified("alice@example.com").await;
    let name = format!("{}.txt", "a".repeat(200));
    upload(&app, &token, &name, b"abc").await;
    let export = |accept_encoding: &str| {
        TestRequest::get("/api/v1/me/export")
            .session(&token)
            .header("accept-encoding", accept_encoding)
    };

    let plain = app.send(export("identity")).await;
    let compressed = app.send(export("gzip")).await;

    assert_eq!(plain.status, StatusCode::OK);
    assert_eq!(plain.body["files"][0]["name"], name);
    assert_eq!(plain.body["files"][0]["size"], 3);
    assert_eq!(compressed.status, StatusCode::OK);
    assert_eq!(compressed.header("content-encoding"), Some("gzip"));
}
