//! Writes problem documents in the language of the request.
//!
//! An [`ApiError`] leaves its handler (or extractor, or layer) as a response with an empty body and
//! itself in the extensions (see `crate::problem`). [`apply`] sits outside everything that can
//! produce one, negotiates the language with the `Accept-Language` header against what the catalog
//! has, and renders the body, so every error path is localized in this one place. The response
//! then carries `Content-Language` and `Vary: Accept-Language`. Responses that are not problems
//! pass through untouched.

use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::header::ACCEPT_LANGUAGE,
    middleware::Next,
    response::Response,
};
use domain::i18n::Translator;

use crate::problem::ApiError;

pub async fn apply(
    State(translator): State<Arc<dyn Translator>>,
    request: Request,
    next: Next,
) -> Response {
    // A reference-counted copy, read only if the response turns out to be a problem.
    let accept_language = request.headers().get(ACCEPT_LANGUAGE).cloned();
    let mut response = next.run(request).await;
    match response.extensions_mut().remove::<ApiError>() {
        Some(error) => {
            let header = accept_language
                .as_ref()
                .and_then(|value| value.to_str().ok());
            let locale = translator.negotiate(header);
            error.render(response, &*translator, &locale)
        }
        None => response,
    }
}
