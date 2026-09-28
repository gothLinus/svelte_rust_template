//! Serves the built SvelteKit app in production.
//!
//! - `/_app/immutable/*` holds content-hashed files: cached for a year, never revalidated. A
//!   missing file there is a real 404, not the SPA fallback.
//! - Everything else is served from the build directory if it exists, and otherwise answered with
//!   `index.html` so client-side routing can take over. These responses are revalidated on every
//!   load (`no-cache`), so a deploy is picked up immediately.
//! - `bun run build` writes a `.br` and a `.gz` next to every compressible file. Those are sent to
//!   browsers that accept them, so nothing static is compressed per request.

use std::path::Path;

use axum::{
    Router,
    extract::Request,
    http::{HeaderValue, header::CACHE_CONTROL},
    middleware::{self, Next},
    response::Response,
};
use tower_http::services::{ServeDir, ServeFile};

const IMMUTABLE_PREFIX: &str = "/_app/immutable/";

pub fn router(dir: &Path) -> Router {
    let index = ServeFile::new(dir.join("index.html"))
        .precompressed_br()
        .precompressed_gzip();
    let assets = ServeDir::new(dir.join("_app"))
        .precompressed_br()
        .precompressed_gzip();
    let files = ServeDir::new(dir)
        .precompressed_br()
        .precompressed_gzip()
        .fallback(index);
    Router::new()
        .nest_service("/_app", assets)
        .fallback_service(files)
        .layer(middleware::from_fn(cache_headers))
}

async fn cache_headers(request: Request, next: Next) -> Response {
    let immutable = request.uri().path().starts_with(IMMUTABLE_PREFIX);
    let mut response = next.run(request).await;

    if response.status().is_success() || response.status().is_redirection() {
        let value = if immutable {
            "public, max-age=31536000, immutable"
        } else {
            "no-cache"
        };
        response
            .headers_mut()
            .insert(CACHE_CONTROL, HeaderValue::from_static(value));
    }
    response
}
