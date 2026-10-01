use std::future::{Future, ready};

use application::Adapters;
use axum::{
    extract::FromRequestParts,
    http::{header::ACCEPT_LANGUAGE, request::Parts},
};
use domain::i18n::Locale;

use crate::state::AppState;

/// The language a request asks for in `Accept-Language`, as the best match the catalog has; `None`
/// when the request names none. Problem documents are localized elsewhere (`middleware::localize`);
/// this is for what a handler stores or words itself.
#[derive(Debug, Clone)]
pub struct Language(pub Option<Locale>);

impl<A: Adapters> FromRequestParts<AppState<A>> for Language {
    type Rejection = std::convert::Infallible;

    fn from_request_parts(
        parts: &mut Parts,
        state: &AppState<A>,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        let locale = parts
            .headers
            .get(ACCEPT_LANGUAGE)
            .and_then(|value| value.to_str().ok())
            .filter(|header| !header.trim().is_empty())
            .map(|header| state.translator.negotiate(Some(header)));
        ready(Ok(Self(locale)))
    }
}
