//! The second sign-in step (`/api/v1/auth/mfa`) and managing it (`/api/v1/me/mfa`).
//!
//! The `/auth/mfa` endpoints finish a sign-in that answered `202`: the attempt is identified by
//! the short-lived `mfa` cookie, not by a session. A correct answer responds like any completed
//! sign-in (session cookie, `mfa` cookie cleared). Every answer is counted per IP and per account
//! (`Action::CheckCode`), however many attempts were started. The `/me/mfa` endpoints need a
//! signed-in user.

use application::{
    Adapters,
    dto::MfaChallengeDto,
    mfa::dto::{CodeRequest, RecoveryCodesDto, SecondFactorAddedDto, TotpSetupDto},
    passkeys::dto::{PasskeyAssertionRequest, PasskeyRequestOptionsDto},
};
use axum::{
    Router,
    extract::State,
    http::StatusCode,
    response::Response,
    routing::{get, post},
};
use axum_extra::extract::CookieJar;
use domain::secret::Secret;

use crate::{
    extract::{Client, CurrentUser, Proto},
    problem::ApiError,
    rate_limit::Action,
    routes::auth::signed_in_response,
    state::AppState,
};

pub fn routes<A: Adapters>() -> Router<AppState<A>> {
    Router::new()
        .route("/auth/mfa", get(pending::<A>))
        .route("/auth/mfa/totp", post(complete_with_totp::<A>))
        .route(
            "/auth/mfa/recovery-code",
            post(complete_with_recovery_code::<A>),
        )
        .route("/auth/mfa/passkeys/options", post(passkey_options::<A>))
        .route("/auth/mfa/passkeys", post(complete_with_passkey::<A>))
        .route(
            "/me/mfa/totp",
            post(start_totp::<A>).delete(remove_totp::<A>),
        )
        .route("/me/mfa/totp/confirm", post(confirm_totp::<A>))
        .route(
            "/me/mfa/recovery-codes",
            post(regenerate_recovery_codes::<A>),
        )
}

/// The pending attempt's token, or an empty one (which the service rejects as expired).
fn attempt<A: Adapters>(state: &AppState<A>, jar: &CookieJar) -> Secret {
    state.mfa_cookie.token(jar).unwrap_or_default()
}

async fn checked_attempt<A: Adapters>(
    state: &AppState<A>,
    client: &Client,
    jar: &CookieJar,
) -> Result<Secret, ApiError> {
    state
        .limits
        .check_ip(Action::CheckCode, client.ip())
        .await?;
    let token = attempt(state, jar);
    let user = state.services.mfa.pending_user(&token).await?;
    state.limits.check_user(Action::CheckCode, user).await?;
    Ok(token)
}

async fn pending<A: Adapters>(
    State(state): State<AppState<A>>,
    jar: CookieJar,
) -> Result<Proto<MfaChallengeDto>, ApiError> {
    Ok(Proto(
        state.services.mfa.pending(&attempt(&state, &jar)).await?,
    ))
}

async fn complete_with_totp<A: Adapters>(
    State(state): State<AppState<A>>,
    client: Client,
    jar: CookieJar,
    Proto(body): Proto<CodeRequest>,
) -> Result<Response, ApiError> {
    let token = checked_attempt(&state, &client, &jar).await?;
    let previous = state.cookie.token(&jar);
    let signed_in = state
        .services
        .mfa
        .complete_with_totp(&token, body, client.0, previous.as_ref())
        .await?;
    Ok(signed_in_response(&state, jar, signed_in))
}

async fn complete_with_recovery_code<A: Adapters>(
    State(state): State<AppState<A>>,
    client: Client,
    jar: CookieJar,
    Proto(body): Proto<CodeRequest>,
) -> Result<Response, ApiError> {
    let token = checked_attempt(&state, &client, &jar).await?;
    let previous = state.cookie.token(&jar);
    let signed_in = state
        .services
        .mfa
        .complete_with_recovery_code(&token, body, client.0, previous.as_ref())
        .await?;
    Ok(signed_in_response(&state, jar, signed_in))
}

async fn passkey_options<A: Adapters>(
    State(state): State<AppState<A>>,
    client: Client,
    jar: CookieJar,
) -> Result<Proto<PasskeyRequestOptionsDto>, ApiError> {
    state.limits.check_ip(Action::Ceremony, client.ip()).await?;
    Ok(Proto(
        state
            .services
            .passkeys
            .second_step_options(&attempt(&state, &jar))
            .await?,
    ))
}

async fn complete_with_passkey<A: Adapters>(
    State(state): State<AppState<A>>,
    client: Client,
    jar: CookieJar,
    Proto(body): Proto<PasskeyAssertionRequest>,
) -> Result<Response, ApiError> {
    let token = checked_attempt(&state, &client, &jar).await?;
    let previous = state.cookie.token(&jar);
    let signed_in = state
        .services
        .passkeys
        .complete_second_step(&token, body, client.0, previous.as_ref())
        .await?;
    Ok(signed_in_response(&state, jar, signed_in))
}

async fn start_totp<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
) -> Result<Proto<TotpSetupDto>, ApiError> {
    Ok(Proto(state.services.mfa.start_totp(user.actor()).await?))
}

async fn confirm_totp<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
    Proto(body): Proto<CodeRequest>,
) -> Result<Proto<SecondFactorAddedDto>, ApiError> {
    state
        .limits
        .check_user(Action::CheckCode, user.actor().user_id)
        .await?;
    Ok(Proto(
        state.services.mfa.confirm_totp(user.actor(), body).await?,
    ))
}

async fn remove_totp<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
    Proto(body): Proto<CodeRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .limits
        .check_user(Action::CheckCode, user.actor().user_id)
        .await?;
    state.services.mfa.remove_totp(user.actor(), body).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn regenerate_recovery_codes<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
) -> Result<Proto<RecoveryCodesDto>, ApiError> {
    Ok(Proto(
        state
            .services
            .mfa
            .regenerate_recovery_codes(user.actor())
            .await?,
    ))
}
