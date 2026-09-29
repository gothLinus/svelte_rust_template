//! Passkeys: signing in with one (`/api/v1/auth/passkeys`) and managing them
//! (`/api/v1/me/passkeys`). Using a passkey as a second step is in `routes/mfa.rs`, and to
//! re-authenticate in `routes/reauth.rs`.

use application::{
    Adapters,
    passkeys::dto::{
        PasskeyAssertionRequest, PasskeyCreationOptionsDto, PasskeyDto, PasskeyRegisteredDto,
        PasskeyRequestOptionsDto, RegisterPasskeyRequest, RenamePasskeyRequest,
    },
};
use axum::{
    Router,
    extract::State,
    http::StatusCode,
    response::Response,
    routing::{get, patch, post},
};
use axum_extra::extract::CookieJar;
use domain::passkey::PasskeyId;
use uuid::Uuid;

use crate::{
    extract::{Client, CurrentUser, Path, Proto},
    problem::ApiError,
    rate_limit::Action,
    routes::auth::signed_in_response,
    state::AppState,
};

pub fn routes<A: Adapters>() -> Router<AppState<A>> {
    Router::new()
        .route("/auth/passkeys/options", post(login_options::<A>))
        .route("/auth/passkeys/login", post(login::<A>))
        .route("/me/passkeys", get(list::<A>).post(register::<A>))
        .route("/me/passkeys/options", post(registration_options::<A>))
        .route("/me/passkeys/{id}", patch(rename::<A>).delete(delete::<A>))
}

async fn login_options<A: Adapters>(
    State(state): State<AppState<A>>,
    client: Client,
) -> Result<Proto<PasskeyRequestOptionsDto>, ApiError> {
    state.limits.check_ip(Action::Ceremony, client.ip()).await?;
    Ok(Proto(state.services.passkeys.login_options().await?))
}

async fn login<A: Adapters>(
    State(state): State<AppState<A>>,
    client: Client,
    jar: CookieJar,
    Proto(body): Proto<PasskeyAssertionRequest>,
) -> Result<Response, ApiError> {
    state.limits.check_ip(Action::Login, client.ip()).await?;
    let previous = state.cookie.token(&jar);
    let signed_in = state
        .services
        .passkeys
        .login(body, client.0, previous.as_ref())
        .await?;
    Ok(signed_in_response(&state, jar, signed_in))
}

async fn list<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
) -> Result<Proto<Vec<PasskeyDto>>, ApiError> {
    Ok(Proto(state.services.passkeys.list(user.actor()).await?))
}

async fn registration_options<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
) -> Result<Proto<PasskeyCreationOptionsDto>, ApiError> {
    Ok(Proto(
        state
            .services
            .passkeys
            .registration_options(user.actor())
            .await?,
    ))
}

async fn register<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
    Proto(body): Proto<RegisterPasskeyRequest>,
) -> Result<(StatusCode, Proto<PasskeyRegisteredDto>), ApiError> {
    let registered = state.services.passkeys.register(user.actor(), body).await?;
    Ok((StatusCode::CREATED, Proto(registered)))
}

async fn rename<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    Proto(body): Proto<RenamePasskeyRequest>,
) -> Result<Proto<PasskeyDto>, ApiError> {
    Ok(Proto(
        state
            .services
            .passkeys
            .rename(user.actor(), PasskeyId::from_uuid(id), body)
            .await?,
    ))
}

async fn delete<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    state
        .services
        .passkeys
        .delete(user.actor(), PasskeyId::from_uuid(id))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
