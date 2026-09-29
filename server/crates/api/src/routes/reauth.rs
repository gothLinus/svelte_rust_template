//! `/api/v1/me/reauthenticate`: proving again who is behind the session before a sensitive change.
//! Those answer `403 reauth_required` when the session has not signed in or re-authenticated
//! recently; the client re-authenticates here and retries.

use application::{
    Adapters,
    auth::dto::{ReauthMethodsDto, ReauthenticateRequest},
    passkeys::dto::{PasskeyAssertionRequest, PasskeyRequestOptionsDto},
};
use axum::{
    Router,
    extract::State,
    http::StatusCode,
    routing::{get, post},
};

use crate::{
    extract::{Client, CurrentUser, Proto},
    problem::ApiError,
    rate_limit::Action,
    state::AppState,
};

pub fn routes<A: Adapters>() -> Router<AppState<A>> {
    Router::new()
        .route(
            "/me/reauthenticate",
            get(methods::<A>).post(reauthenticate::<A>),
        )
        .route("/me/reauthenticate/email-code", post(send_email_code::<A>))
        .route(
            "/me/reauthenticate/passkey/options",
            post(passkey_options::<A>),
        )
        .route("/me/reauthenticate/passkey", post(with_passkey::<A>))
}

async fn methods<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
) -> Result<Proto<ReauthMethodsDto>, ApiError> {
    Ok(Proto(state.services.reauth.methods(user.actor()).await?))
}

async fn reauthenticate<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
    client: Client,
    Proto(body): Proto<ReauthenticateRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .limits
        .check_ip(Action::CheckCode, client.ip())
        .await?;
    state
        .limits
        .check_user(Action::CheckCode, user.actor().user_id)
        .await?;
    state
        .services
        .reauth
        .reauthenticate(user.actor(), body)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn send_email_code<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
    client: Client,
) -> Result<StatusCode, ApiError> {
    state.limits.check_ip(Action::SendCode, client.ip()).await?;
    state
        .limits
        .check_user(Action::SendCode, user.actor().user_id)
        .await?;
    state.services.reauth.send_email_code(user.actor()).await?;
    Ok(StatusCode::ACCEPTED)
}

async fn passkey_options<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
    client: Client,
) -> Result<Proto<PasskeyRequestOptionsDto>, ApiError> {
    state.limits.check_ip(Action::Ceremony, client.ip()).await?;
    Ok(Proto(
        state
            .services
            .passkeys
            .reauthentication_options(user.actor())
            .await?,
    ))
}

async fn with_passkey<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
    client: Client,
    Proto(body): Proto<PasskeyAssertionRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .limits
        .check_ip(Action::CheckCode, client.ip())
        .await?;
    state
        .limits
        .check_user(Action::CheckCode, user.actor().user_id)
        .await?;
    state
        .services
        .passkeys
        .reauthenticate(user.actor(), body)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
