//! Resolves the session cookie once per request.
//!
//! This middleware never rejects a request for lacking a session; it only records who the caller
//! is. Requiring one is the extractors' job (`CurrentUser`, `RequirePermission`).

use std::sync::Arc;

use application::Adapters;
use axum::{
    RequestPartsExt,
    extract::{Request, State},
    middleware::Next,
    response::{IntoResponse, Response},
};
use axum_extra::extract::CookieJar;

use crate::{
    extract::{Authentication, Client},
    problem::ApiError,
    state::AppState,
};

/// Looks up the session behind the cookie and stores it as an [`Authentication`] extension for the
/// extractors. Afterwards it keeps the browser's cookie in sync:
///
/// - a stale cookie (expired, revoked, unknown) is cleared;
/// - a rotated token (the user's roles changed) is sent as a new cookie.
///
/// A handler that sets the cookie itself (sign-in, sign-out) always wins.
pub async fn authenticate<A: Adapters>(
    State(state): State<AppState<A>>,
    mut request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    let jar = CookieJar::from_headers(request.headers());
    let token = state.cookie.token(&jar);

    let mut rotated = None;
    let mut clear_cookie = false;
    if let Some(token) = &token {
        let (mut parts, body) = request.into_parts();
        let Ok(Client(client)) = parts.extract_with_state::<Client, _>(&state).await;
        request = Request::from_parts(parts, body);
        match state.services.auth.authenticate(token, client).await? {
            Some(authenticated) => {
                rotated.clone_from(&authenticated.rotated_token);
                request
                    .extensions_mut()
                    .insert(Authentication(Arc::new(authenticated)));
            }
            None => clear_cookie = true,
        }
    }

    let response = next.run(request).await;
    if state.cookie.is_set_in(response.headers()) {
        return Ok(response);
    }

    // Built from the request's jar: a removal is only emitted for a cookie the browser actually
    // sent.
    let jar = match (rotated, clear_cookie) {
        (Some(token), _) => state.cookie.set(jar, &token),
        (None, true) => state.cookie.clear(jar),
        (None, false) => return Ok(response),
    };
    Ok((jar, response).into_response())
}
