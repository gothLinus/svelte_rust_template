//! `/api/v1/admin`: account administration.
//!
//! [`RequirePermission`] rejects callers without the permission before the handler runs; the
//! service checks again, so the rule holds for any other caller of the service too.

use application::{
    Adapters,
    admin::dto::ListUsersQuery,
    dto::{RoleDto, UserDto},
    pagination::PageDto,
};
use axum::{
    Router,
    extract::State,
    routing::{get, post, put},
};
use domain::user::UserId;
use uuid::Uuid;

use crate::{
    extract::{Path, Proto, Query, RequirePermission, permission},
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
        .route("/admin/roles", get(list_roles::<A>))
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
) -> Result<Proto<Vec<RoleDto>>, ApiError> {
    Ok(Proto(state.services.admin.list_roles(admin.actor()).await?))
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
