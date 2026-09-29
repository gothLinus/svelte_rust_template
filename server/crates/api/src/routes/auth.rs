//! `/api/v1/auth`: registration, sign-in and sign-out, email verification, password reset.
//!
//! Which cookie a response sets or clears is part of the contract:
//!
//! - A completed sign-in sets the session cookie and the known-device mark, and clears any pending
//!   `mfa` cookie (`signed_in_response`). When the account has a second step, the sign-in answers
//!   `202` with the `MfaChallenge` and sets the `mfa` cookie instead (`login_response`);
//!   `routes/mfa.rs` completes it. Passwordless routes reuse `login_response`, passkey and
//!   second-step routes `signed_in_response`.
//! - Registering an account whose address must be verified first sets the `registration` cookie,
//!   not a session.
//! - Signing out always clears the session cookie.
//!
//! Handlers that must not reveal whether an account exists answer `202` before doing their work
//! (see `Background`). Each handler counts itself against the `Action` rate limits it names; the
//! per-IP API limit already applies to the whole group.

use std::sync::Arc;

use application::{
    Adapters, AppError,
    auth::{
        Browser, LoginOutcome, Registered, SignedIn,
        dto::{
            AuthMethodsDto, CancelEmailChangeRequest, ConfirmEmailRequest, ForgotPasswordRequest,
            LoginRequest, RegisterRequest, ResetPasswordRequest, VerifyEmailRequest,
        },
    },
};
use axum::{
    Router,
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
};
use axum_extra::extract::CookieJar;
use domain::user::{Email, UserId};

use crate::{
    extract::{Client, CurrentUser, Proto},
    problem::ApiError,
    rate_limit::{Action, account_key, user_key},
    state::AppState,
};

pub fn routes<A: Adapters>() -> Router<AppState<A>> {
    Router::new()
        .route("/auth/methods", get(methods::<A>))
        .route("/auth/register", post(register::<A>))
        .route("/auth/login", post(login::<A>))
        .route("/auth/logout", post(logout::<A>))
        .route("/auth/logout-all", post(logout_all::<A>))
        .route("/auth/verify-email", post(verify_email::<A>))
        .route("/auth/verify-email/resend", post(resend_verification::<A>))
        .route("/auth/forgot-password", post(forgot_password::<A>))
        .route("/auth/reset-password", post(reset_password::<A>))
        .route("/auth/confirm-email", post(confirm_email::<A>))
        .route("/auth/cancel-email-change", post(cancel_email_change::<A>))
}

async fn methods<A: Adapters>(State(state): State<AppState<A>>) -> Proto<AuthMethodsDto> {
    Proto(state.services.auth.methods())
}

/// `200` with the user and a session cookie, or `202` with the second-step methods and a
/// short-lived cookie that identifies the attempt.
pub(super) fn login_response<A: Adapters>(
    state: &AppState<A>,
    jar: CookieJar,
    outcome: LoginOutcome,
) -> Response {
    match outcome {
        LoginOutcome::SignedIn(signed_in) => signed_in_response(state, jar, *signed_in),
        LoginOutcome::MfaRequired(required) => {
            let jar = state.mfa_cookie.set(jar, &required.token);
            (StatusCode::ACCEPTED, jar, Proto(required.challenge)).into_response()
        }
    }
}

pub(super) fn signed_in_response<A: Adapters>(
    state: &AppState<A>,
    jar: CookieJar,
    signed_in: SignedIn,
) -> Response {
    let jar = signed_in_cookies(state, jar, &signed_in);
    let jar = state.mfa_cookie.clear(jar);
    (jar, Proto(signed_in.me)).into_response()
}

fn signed_in_cookies<A: Adapters>(
    state: &AppState<A>,
    jar: CookieJar,
    signed_in: &SignedIn,
) -> CookieJar {
    let user = UserId::from_uuid(signed_in.me.user.id);
    let jar = state.cookie.set(jar, &signed_in.token);
    state
        .device_cookie
        .set(jar, &state.services.auth.known_device_mark(user))
}

/// `201` with the new user and a session cookie, or `202` when the address must be verified before
/// signing in.
async fn register<A: Adapters>(
    State(state): State<AppState<A>>,
    client: Client,
    jar: CookieJar,
    Proto(body): Proto<RegisterRequest>,
) -> Result<Response, ApiError> {
    state.limits.check_ip(Action::Register, client.ip()).await?;
    // Every attempt mails the address (a verification link, or "you already have an account"), so
    // the address gets a budget of its own: rotating IPs cannot flood it.
    state
        .limits
        .check_account(Action::SendCode, &body.email)
        .await?;

    match state.services.auth.register(body, client.0).await? {
        Registered::SignedIn(signed_in) => {
            let jar = signed_in_cookies(&state, jar, &signed_in);
            Ok((StatusCode::CREATED, jar, Proto(signed_in.me)).into_response())
        }
        Registered::VerificationPending { pending, browser } => {
            let jar = state.registration_cookie.set(jar, &browser);
            Ok((StatusCode::ACCEPTED, jar, Proto(pending)).into_response())
        }
    }
}

/// Password sign-in: `200` or `202` as in [`login_response`]. Counted per IP and per account; only
/// failed passwords stay counted, and the account-wide ceiling is skipped for a browser carrying
/// the known-device mark.
async fn login<A: Adapters>(
    State(state): State<AppState<A>>,
    client: Client,
    jar: CookieJar,
    Proto(body): Proto<LoginRequest>,
) -> Result<Response, ApiError> {
    state.limits.check_ip(Action::Login, client.ip()).await?;
    let account = state.services.auth.login_account(&body.identifier).await?;
    let (key, known_device) = match account.id() {
        Some(user) => (
            user_key(user),
            state
                .device_cookie
                .token(&jar)
                .is_some_and(|mark| state.services.auth.is_known_device(user, &mark)),
        ),
        None => (account_key(&body.identifier), false),
    };
    let attempt = state
        .limits
        .start_login(key, client.ip(), known_device)
        .await?;

    let previous = state.cookie.token(&jar);
    let outcome = state
        .services
        .auth
        .login_as(account, &body.password.0, client.0, previous.as_ref())
        .await?;
    // Only failed passwords count against the account.
    state.limits.login_succeeded(attempt).await;
    Ok(login_response(&state, jar, outcome))
}

async fn logout<A: Adapters>(
    State(state): State<AppState<A>>,
    jar: CookieJar,
) -> Result<(CookieJar, StatusCode), ApiError> {
    if let Some(token) = state.cookie.token(&jar) {
        state.services.auth.logout(&token).await?;
    }
    Ok((state.cookie.clear(jar), StatusCode::NO_CONTENT))
}

async fn logout_all<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
    jar: CookieJar,
) -> Result<(CookieJar, StatusCode), ApiError> {
    state.services.auth.logout_everywhere(user.actor()).await?;
    Ok((state.cookie.clear(jar), StatusCode::NO_CONTENT))
}

async fn verify_email<A: Adapters>(
    State(state): State<AppState<A>>,
    client: Client,
    jar: CookieJar,
    Proto(body): Proto<VerifyEmailRequest>,
) -> Result<(CookieJar, StatusCode), ApiError> {
    state
        .limits
        .check_ip(Action::Verification, client.ip())
        .await?;
    let session = state.cookie.token(&jar);
    let registration = state.registration_cookie.token(&jar);
    state
        .services
        .auth
        .verify_email(
            body,
            Browser {
                session: session.as_ref(),
                registration: registration.as_ref(),
            },
        )
        .await?;
    Ok((state.registration_cookie.clear(jar), StatusCode::NO_CONTENT))
}

async fn resend_verification<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
) -> Result<StatusCode, ApiError> {
    state
        .limits
        .check_user(Action::Verification, user.actor().user_id)
        .await?;
    state
        .services
        .auth
        .resend_verification(user.actor())
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Always `202`, whether or not the address has an account, and answered before the link is issued
/// and mailed, so the response time does not tell either.
async fn forgot_password<A: Adapters>(
    State(state): State<AppState<A>>,
    client: Client,
    Proto(body): Proto<ForgotPasswordRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .limits
        .check_ip(Action::PasswordReset, client.ip())
        .await?;
    Email::parse(&body.email).map_err(|err| AppError::invalid("email", &err))?;
    state
        .limits
        .check_account(Action::PasswordReset, &body.email)
        .await?;
    let services = Arc::clone(&state.services);
    state
        .background
        .run(async move {
            if let Err(err) = services.auth.request_password_reset(body).await {
                ApiError::log_detached(err);
            }
        })
        .await;
    Ok(StatusCode::ACCEPTED)
}

async fn reset_password<A: Adapters>(
    State(state): State<AppState<A>>,
    client: Client,
    Proto(body): Proto<ResetPasswordRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .limits
        .check_ip(Action::PasswordReset, client.ip())
        .await?;
    state.services.auth.reset_password(body).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn confirm_email<A: Adapters>(
    State(state): State<AppState<A>>,
    client: Client,
    jar: CookieJar,
    Proto(body): Proto<ConfirmEmailRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .limits
        .check_ip(Action::Verification, client.ip())
        .await?;
    let current = state.cookie.token(&jar);
    state
        .services
        .auth
        .confirm_email_change(body, current.as_ref())
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn cancel_email_change<A: Adapters>(
    State(state): State<AppState<A>>,
    client: Client,
    Proto(body): Proto<CancelEmailChangeRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .limits
        .check_ip(Action::Verification, client.ip())
        .await?;
    state.services.auth.cancel_email_change(body).await?;
    Ok(StatusCode::NO_CONTENT)
}
