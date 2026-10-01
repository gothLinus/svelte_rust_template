//! `/api/v1/me`: the signed-in user's own account.
//!
//! Every endpoint needs a session. Those that end the current one (deleting the account, revoking
//! one's own session) also clear its cookie.

use application::{
    Adapters,
    account::dto::{
        AddPhoneRequest, ChangeEmailRequest, DeleteAccountRequest, SecurityDto, SetLocaleRequest,
        UpdateProfileRequest,
    },
    audit::dto::{AuditEventDto, ListActivityQuery},
    dto::{MeDto, SessionDto},
    mfa::dto::CodeRequest,
    pagination::PageDto,
};
use axum::{
    Json, Router,
    extract::State,
    http::{StatusCode, header::CONTENT_DISPOSITION},
    response::IntoResponse,
    routing::{delete, get, post, put},
};
use axum_extra::extract::CookieJar;
use domain::session::SessionId;
use uuid::Uuid;

use crate::{
    extract::{Client, CurrentUser, Path, Proto, Query},
    problem::ApiError,
    rate_limit::Action,
    state::AppState,
};

pub fn routes<A: Adapters>() -> Router<AppState<A>> {
    Router::new()
        .route(
            "/me",
            get(me::<A>)
                .patch(update_profile::<A>)
                .delete(delete_account::<A>),
        )
        .route("/me/locale", put(set_locale::<A>))
        .route("/me/email", post(change_email::<A>))
        .route("/me/password", post(change_password::<A>))
        .route("/me/phone", post(add_phone::<A>).delete(remove_phone::<A>))
        .route("/me/phone/verify", post(verify_phone::<A>))
        .route("/me/security", get(security::<A>))
        .route("/me/export", get(export::<A>))
        .route("/me/sessions", get(sessions::<A>))
        .route("/me/sessions/{id}", delete(revoke_session::<A>))
        .route("/me/activity", get(activity::<A>))
}

async fn set_locale<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
    Proto(body): Proto<SetLocaleRequest>,
) -> Result<Proto<MeDto>, ApiError> {
    Ok(Proto(
        state
            .services
            .account
            .set_locale(user.actor(), body)
            .await?,
    ))
}

async fn activity<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
    Query(query): Query<ListActivityQuery>,
) -> Result<Proto<PageDto<AuditEventDto>>, ApiError> {
    Ok(Proto(
        state.services.audit.activity(user.actor(), query).await?,
    ))
}

async fn export<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
) -> Result<impl IntoResponse, ApiError> {
    let export = state.services.account.export(user.actor()).await?;
    Ok((
        [(
            CONTENT_DISPOSITION,
            "attachment; filename=\"personal-data.json\"",
        )],
        Json(export),
    ))
}

async fn me<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
) -> Result<Proto<MeDto>, ApiError> {
    Ok(Proto(state.services.account.me(user.user()).await?))
}

async fn update_profile<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
    Proto(body): Proto<UpdateProfileRequest>,
) -> Result<Proto<MeDto>, ApiError> {
    Ok(Proto(
        state
            .services
            .account
            .update_profile(user.actor(), body)
            .await?,
    ))
}

async fn change_email<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
    Proto(body): Proto<ChangeEmailRequest>,
) -> Result<StatusCode, ApiError> {
    state
        .limits
        .check_user(Action::SendCode, user.actor().user_id)
        .await?;
    state
        .services
        .account
        .request_email_change(user.actor(), body)
        .await?;
    Ok(StatusCode::ACCEPTED)
}

async fn change_password<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
) -> Result<StatusCode, ApiError> {
    state
        .limits
        .check_user(Action::SendCode, user.actor().user_id)
        .await?;
    state
        .services
        .account
        .request_password_change(user.actor())
        .await?;
    Ok(StatusCode::ACCEPTED)
}

/// Texts a verification code to a new number; `202`. Limited per user, per client IP and per
/// destination number, so throwaway accounts cannot pump texts to premium numbers on the
/// operator's bill.
async fn add_phone<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
    client: Client,
    Proto(body): Proto<AddPhoneRequest>,
) -> Result<StatusCode, ApiError> {
    state.limits.check_ip(Action::SendCode, client.ip()).await?;
    state
        .limits
        .check_user(Action::SendCode, user.actor().user_id)
        .await?;
    state
        .limits
        .check_account(Action::SendCode, &body.phone)
        .await?;
    state.services.account.add_phone(user.actor(), body).await?;
    Ok(StatusCode::ACCEPTED)
}

async fn verify_phone<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
    Proto(body): Proto<CodeRequest>,
) -> Result<Proto<MeDto>, ApiError> {
    state
        .limits
        .check_user(Action::CheckCode, user.actor().user_id)
        .await?;
    Ok(Proto(
        state
            .services
            .account
            .verify_phone(user.actor(), body)
            .await?,
    ))
}

async fn remove_phone<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
) -> Result<Proto<MeDto>, ApiError> {
    Ok(Proto(
        state.services.account.remove_phone(user.actor()).await?,
    ))
}

async fn security<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
) -> Result<Proto<SecurityDto>, ApiError> {
    Ok(Proto(state.services.account.security(user.actor()).await?))
}

async fn sessions<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
) -> Result<Proto<Vec<SessionDto>>, ApiError> {
    Ok(Proto(state.services.account.sessions(user.actor()).await?))
}

async fn revoke_session<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
    jar: CookieJar,
    Path(id): Path<Uuid>,
) -> Result<(CookieJar, StatusCode), ApiError> {
    let id = SessionId::from_uuid(id);
    state
        .services
        .account
        .revoke_session(user.actor(), id)
        .await?;

    let jar = if id == user.session().id() {
        state.cookie.clear(jar)
    } else {
        jar
    };
    Ok((jar, StatusCode::NO_CONTENT))
}

async fn delete_account<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
    jar: CookieJar,
    Proto(body): Proto<DeleteAccountRequest>,
) -> Result<(CookieJar, StatusCode), ApiError> {
    state
        .limits
        .check_user(Action::CheckCode, user.actor().user_id)
        .await?;
    state
        .services
        .account
        .delete_account(user.actor(), body)
        .await?;
    Ok((state.cookie.clear(jar), StatusCode::NO_CONTENT))
}
