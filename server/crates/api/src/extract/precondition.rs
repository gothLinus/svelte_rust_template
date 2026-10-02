//! Optimistic concurrency over HTTP: an entity's [`Version`] goes out as a strong `ETag` (`"3"`)
//! and comes back in `If-Match`, so an update or delete made against an outdated copy is refused
//! with `412` instead of silently overwriting someone else's change.

use std::future::{Future, ready};

use application::AppError;
use axum::{
    extract::FromRequestParts,
    http::{
        HeaderValue,
        header::{ETAG, IF_MATCH},
        request::Parts,
    },
    response::{IntoResponseParts, ResponseParts},
};
use domain::repository::Version;

use crate::problem::ApiError;

/// The version a request's `If-Match` names, or `None` when it sent none or `*` (any version).
///
/// Anything else that is not one strong tag this server could have issued, a weak `W/"3"` or a
/// list included, can never match, so it is rejected with `412` as RFC 9110 asks.
#[derive(Debug, Clone, Copy)]
pub struct IfMatch(pub Option<Version>);

impl<S: Send + Sync> FromRequestParts<S> for IfMatch {
    type Rejection = ApiError;

    fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> impl Future<Output = Result<Self, Self::Rejection>> + Send {
        let result = match parts.headers.get(IF_MATCH) {
            None => Ok(Self(None)),
            Some(value) => parse(value).ok_or_else(|| AppError::Stale.into()),
        };
        ready(result)
    }
}

fn parse(value: &HeaderValue) -> Option<IfMatch> {
    let raw = value.to_str().ok()?.trim();
    if raw == "*" {
        return Some(IfMatch(None));
    }
    let digits = raw.strip_prefix('"')?.strip_suffix('"')?;
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    digits
        .parse()
        .ok()
        .map(|raw| IfMatch(Some(Version::new(raw))))
}

/// Sets `ETag` to the entity's version, for the responses that return one entity.
#[derive(Debug, Clone, Copy)]
pub struct ETag(pub Version);

impl IntoResponseParts for ETag {
    type Error = std::convert::Infallible;

    fn into_response_parts(self, mut parts: ResponseParts) -> Result<ResponseParts, Self::Error> {
        // Digits in quotes, always a valid header value.
        if let Ok(value) = HeaderValue::try_from(format!("\"{}\"", self.0.get())) {
            parts.headers_mut().insert(ETAG, value);
        }
        Ok(parts)
    }
}
