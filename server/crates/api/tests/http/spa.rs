use std::{fs, path::PathBuf};

use axum::http::StatusCode;
use sqlx::PgPool;

use crate::support::{Options, TestApp, TestRequest};

const INDEX: &str = "<!doctype html><title>app</title>";
const ASSET: &str = "export const answer = 42;";

fn build_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("spa-{}", uuid::Uuid::now_v7()));
    fs::create_dir_all(dir.join("_app/immutable/chunks")).unwrap();
    fs::write(dir.join("index.html"), INDEX).unwrap();
    fs::write(dir.join("robots.txt"), "User-agent: *").unwrap();
    fs::write(dir.join("_app/immutable/chunks/app.abc123.js"), ASSET).unwrap();
    dir
}

fn app(pool: PgPool, dir: &std::path::Path) -> TestApp {
    TestApp::with(
        pool,
        Options {
            static_dir: Some(dir.to_path_buf()),
            ..Options::default()
        },
    )
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn client_routes_fall_back_to_index_html(pool: PgPool) {
    let dir = build_dir();
    let app = app(pool, &dir);

    for path in ["/", "/notes", "/admin/users/123"] {
        let response = app.send(TestRequest::get(path)).await;
        assert_eq!(response.status, StatusCode::OK, "{path}");
        assert_eq!(response.text, INDEX);
        assert_eq!(response.header("cache-control"), Some("no-cache"));
        let csp = response.header("content-security-policy").unwrap();
        for directive in [
            "frame-ancestors 'none'",
            "object-src 'none'",
            "style-src-elem 'self'",
            "style-src-attr 'unsafe-inline'",
            "report-to csp",
        ] {
            assert!(csp.contains(directive), "{directive} missing from {csp}");
        }
        assert!(
            response
                .header("content-security-policy-report-only")
                .unwrap()
                .contains("require-trusted-types-for 'script'")
        );
        assert_eq!(
            response.header("reporting-endpoints"),
            Some("csp=\"/csp-reports\"")
        );
    }

    let robots = app.send(TestRequest::get("/robots.txt")).await;
    assert_eq!(robots.text, "User-agent: *");
    fs::remove_dir_all(dir).unwrap();
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn hashed_assets_are_cached_forever_and_missing_ones_are_404(pool: PgPool) {
    let dir = build_dir();
    let app = app(pool, &dir);

    let asset = app
        .send(TestRequest::get("/_app/immutable/chunks/app.abc123.js"))
        .await;
    assert_eq!(asset.status, StatusCode::OK);
    assert_eq!(asset.text, ASSET);
    assert_eq!(
        asset.header("cache-control"),
        Some("public, max-age=31536000, immutable")
    );

    let missing = app
        .send(TestRequest::get("/_app/immutable/chunks/gone.js"))
        .await;
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
    assert!(missing.header("cache-control").is_none());
    fs::remove_dir_all(dir).unwrap();
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn the_api_never_falls_back_to_the_spa(pool: PgPool) {
    let dir = build_dir();
    let app = app(pool, &dir);

    app.send(TestRequest::get("/api/v1/unknown"))
        .await
        .assert_problem(StatusCode::NOT_FOUND, "not_found");
    app.send(TestRequest::get("/api/v9"))
        .await
        .assert_problem(StatusCode::NOT_FOUND, "not_found");
    fs::remove_dir_all(dir).unwrap();
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn precompressed_files_are_sent_to_browsers_that_accept_them(pool: PgPool) {
    let dir = build_dir();
    // Stand-ins: the server passes the files through without looking inside.
    fs::write(dir.join("index.html.br"), "brotli index").unwrap();
    fs::write(
        dir.join("_app/immutable/chunks/app.abc123.js.gz"),
        "gzip asset",
    )
    .unwrap();
    let app = app(pool, &dir);

    let index = app
        .send(TestRequest::get("/notes").header("accept-encoding", "br, gzip"))
        .await;
    assert_eq!(index.header("content-encoding"), Some("br"));
    assert_eq!(index.text, "brotli index");
    assert_eq!(index.header("cache-control"), Some("no-cache"));

    let asset = app
        .send(
            TestRequest::get("/_app/immutable/chunks/app.abc123.js")
                .header("accept-encoding", "gzip"),
        )
        .await;
    assert_eq!(asset.header("content-encoding"), Some("gzip"));
    assert_eq!(asset.text, "gzip asset");

    let plain = app.send(TestRequest::get("/robots.txt")).await;
    assert!(plain.header("content-encoding").is_none());
    assert_eq!(plain.text, "User-agent: *");
    fs::remove_dir_all(dir).unwrap();
}
