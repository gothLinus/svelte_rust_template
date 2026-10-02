//! `/api/v1/admin`: account administration.
//!
//! [`RequirePermission`] rejects callers without the permission before the handler runs; the
//! service checks again, so the rule holds for any other caller of the service too.

use application::{
    Adapters,
    admin::dto::ListUsersQuery,
    audit::dto::{AuditEventDto, ListAuditQuery},
    dto::{RoleDto, SessionDto, UserDto},
    pagination::PageDto,
};
use axum::{
    Router,
    extract::State,
    http::StatusCode,
    routing::{delete, get, post, put},
};
use domain::{session::SessionId, user::UserId};
use uuid::Uuid;

use crate::{
    extract::{Language, Path, Proto, Query, RequirePermission, permission},
    problem::ApiError,
    state::AppState,
};

pub fn routes<A: Adapters>() -> Router<AppState<A>> {
    Router::new()
        .route("/admin/users", get(list_users::<A>))
        .route("/admin/users/{id}", get(show_user::<A>))
        .route(
            "/admin/users/{id}/roles/{role}",
            put(grant_role::<A>).delete(revoke_role::<A>),
        )
        .route("/admin/users/{id}/disable", post(disable::<A>))
        .route("/admin/users/{id}/enable", post(enable::<A>))
        .route(
            "/admin/users/{id}/sessions",
            get(user_sessions::<A>).delete(sign_out_user::<A>),
        )
        .route(
            "/admin/users/{id}/sessions/{session}",
            delete(revoke_user_session::<A>),
        )
        .route("/admin/roles", get(list_roles::<A>))
        .route("/admin/audit", get(list_audit_events::<A>))
}

async fn list_audit_events<A: Adapters>(
    State(state): State<AppState<A>>,
    admin: RequirePermission<permission::AuditRead>,
    Query(query): Query<ListAuditQuery>,
) -> Result<Proto<PageDto<AuditEventDto>>, ApiError> {
    Ok(Proto(
        state.services.audit.list(admin.actor(), query).await?,
    ))
}

async fn list_users<A: Adapters>(
    State(state): State<AppState<A>>,
    admin: RequirePermission<permission::UsersRead>,
    Query(query): Query<ListUsersQuery>,
) -> Result<Proto<PageDto<UserDto>>, ApiError> {
    Ok(Proto(
        state
            .services
            .admin
            .list_users(admin.actor(), query)
            .await?,
    ))
}

async fn show_user<A: Adapters>(
    State(state): State<AppState<A>>,
    admin: RequirePermission<permission::UsersRead>,
    Path(id): Path<Uuid>,
) -> Result<Proto<UserDto>, ApiError> {
    Ok(Proto(
        state
            .services
            .admin
            .get_user(admin.actor(), UserId::from_uuid(id))
            .await?,
    ))
}

async fn list_roles<A: Adapters>(
    State(state): State<AppState<A>>,
    admin: RequirePermission<permission::UsersRead>,
    Language(locale): Language,
) -> Result<Proto<Vec<RoleDto>>, ApiError> {
    let locale = locale.unwrap_or_else(|| state.translator.negotiate(None));
    Ok(Proto(
        state
            .services
            .admin
            .list_roles(admin.actor(), &locale)
            .await?,
    ))
}

async fn grant_role<A: Adapters>(
    State(state): State<AppState<A>>,
    admin: RequirePermission<permission::UsersManage>,
    Path((id, role)): Path<(Uuid, String)>,
) -> Result<Proto<UserDto>, ApiError> {
    Ok(Proto(
        state
            .services
            .admin
            .grant_role(admin.actor(), UserId::from_uuid(id), &role)
            .await?,
    ))
}

async fn revoke_role<A: Adapters>(
    State(state): State<AppState<A>>,
    admin: RequirePermission<permission::UsersManage>,
    Path((id, role)): Path<(Uuid, String)>,
) -> Result<Proto<UserDto>, ApiError> {
    Ok(Proto(
        state
            .services
            .admin
            .revoke_role(admin.actor(), UserId::from_uuid(id), &role)
            .await?,
    ))
}

async fn disable<A: Adapters>(
    State(state): State<AppState<A>>,
    admin: RequirePermission<permission::UsersManage>,
    Path(id): Path<Uuid>,
) -> Result<Proto<UserDto>, ApiError> {
    Ok(Proto(
        state
            .services
            .admin
            .set_status(
                admin.actor(),
                UserId::from_uuid(id),
                application::admin::AccountStatus::Disabled,
            )
            .await?,
    ))
}

async fn enable<A: Adapters>(
    State(state): State<AppState<A>>,
    admin: RequirePermission<permission::UsersManage>,
    Path(id): Path<Uuid>,
) -> Result<Proto<UserDto>, ApiError> {
    Ok(Proto(
        state
            .services
            .admin
            .set_status(
                admin.actor(),
                UserId::from_uuid(id),
                application::admin::AccountStatus::Enabled,
            )
            .await?,
    ))
}

async fn user_sessions<A: Adapters>(
    State(state): State<AppState<A>>,
    admin: RequirePermission<permission::UsersRead>,
    Path(id): Path<Uuid>,
) -> Result<Proto<Vec<SessionDto>>, ApiError> {
    Ok(Proto(
        state
            .services
            .admin
            .user_sessions(admin.actor(), UserId::from_uuid(id))
            .await?,
    ))
}

async fn revoke_user_session<A: Adapters>(
    State(state): State<AppState<A>>,
    admin: RequirePermission<permission::UsersManage>,
    Path((id, session)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, ApiError> {
    state
        .services
        .admin
        .revoke_user_session(
            admin.actor(),
            UserId::from_uuid(id),
            SessionId::from_uuid(session),
        )
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn sign_out_user<A: Adapters>(
    State(state): State<AppState<A>>,
    admin: RequirePermission<permission::UsersManage>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    state
        .services
        .admin
        .sign_out_user(admin.actor(), UserId::from_uuid(id))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
