//! `/api/v1/auth/{email-code,magic-link,phone-code}`: signing in without a password.
//!
//! Requesting a code answers `202` whether or not the account exists, with the work handed to
//! `Background`. Verifying one responds like a password sign-in (`login_response`): `200` with the
//! session cookie, or `202` with the `MfaChallenge` when a second step is due.

use std::sync::Arc;

use application::{
    Adapters, AppError,
    passwordless::dto::{
        EmailCodeRequest, MagicLinkRequest, PhoneCodeRequest, VerifyEmailCodeRequest,
        VerifyPhoneCodeRequest,
    },
};
use axum::{Router, extract::State, http::StatusCode, response::Response, routing::post};
use axum_extra::extract::CookieJar;
use domain::user::Email;

use crate::{
    extract::{Client, Proto},
    problem::ApiError,
    rate_limit::Action,
    routes::auth::login_response,
    state::AppState,
};

pub fn routes<A: Adapters>() -> Router<AppState<A>> {
    Router::new()
        .route("/auth/email-code", post(request_email_code::<A>))
        .route("/auth/email-code/verify", post(verify_email_code::<A>))
        .route("/auth/magic-link", post(magic_link::<A>))
        .route("/auth/phone-code", post(request_phone_code::<A>))
        .route("/auth/phone-code/verify", post(verify_phone_code::<A>))
}

async fn request_email_code<A: Adapters>(
    State(state): State<AppState<A>>,
    client: Client,
    Proto(body): Proto<EmailCodeRequest>,
) -> Result<StatusCode, ApiError> {
    state.limits.check_ip(Action::SendCode, client.ip()).await?;
    Email::parse(&body.email).map_err(|err| AppError::invalid("email", &err))?;
    state
        .limits
        .check_account(Action::SendCode, &body.email)
        .await?;
    let services = Arc::clone(&state.services);
    state
        .background
        .run(async move {
            if let Err(err) = services.passwordless.request_email_code(body).await {
                ApiError::log_detached(err);
            }
        })
        .await;
    Ok(StatusCode::ACCEPTED)
}

async fn verify_email_code<A: Adapters>(
    State(state): State<AppState<A>>,
    client: Client,
    jar: CookieJar,
    Proto(body): Proto<VerifyEmailCodeRequest>,
) -> Result<Response, ApiError> {
    state
        .limits
        .check_ip(Action::CheckCode, client.ip())
        .await?;
    state
        .limits
        .check_account(Action::CheckCode, &body.email)
        .await?;
    let previous = state.cookie.token(&jar);
    let outcome = state
        .services
        .passwordless
        .verify_email_code(body, client.0, previous.as_ref())
        .await?;
    Ok(login_response(&state, jar, outcome))
}

async fn magic_link<A: Adapters>(
    State(state): State<AppState<A>>,
    client: Client,
    jar: CookieJar,
    Proto(body): Proto<MagicLinkRequest>,
) -> Result<Response, ApiError> {
    state
        .limits
        .check_ip(Action::CheckCode, client.ip())
        .await?;
    let previous = state.cookie.token(&jar);
    let outcome = state
        .services
        .passwordless
        .magic_link(body, client.0, previous.as_ref())
        .await?;
    Ok(login_response(&state, jar, outcome))
}

async fn request_phone_code<A: Adapters>(
    State(state): State<AppState<A>>,
    client: Client,
    Proto(body): Proto<PhoneCodeRequest>,
) -> Result<StatusCode, ApiError> {
    state.limits.check_ip(Action::SendCode, client.ip()).await?;
    state.services.passwordless.check_phone_request(&body)?;
    state
        .limits
        .check_account(Action::SendCode, &body.phone)
        .await?;
    let services = Arc::clone(&state.services);
    state
        .background
        .run(async move {
            if let Err(err) = services.passwordless.request_phone_code(body).await {
                ApiError::log_detached(err);
            }
        })
        .await;
    Ok(StatusCode::ACCEPTED)
}

async fn verify_phone_code<A: Adapters>(
    State(state): State<AppState<A>>,
    client: Client,
    jar: CookieJar,
    Proto(body): Proto<VerifyPhoneCodeRequest>,
) -> Result<Response, ApiError> {
    state
        .limits
        .check_ip(Action::CheckCode, client.ip())
        .await?;
    state
        .limits
        .check_account(Action::CheckCode, &body.phone)
        .await?;
    let previous = state.cookie.token(&jar);
    let outcome = state
        .services
        .passwordless
        .verify_phone_code(body, client.0, previous.as_ref())
        .await?;
    Ok(login_response(&state, jar, outcome))
}
