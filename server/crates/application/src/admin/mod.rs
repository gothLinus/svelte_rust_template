//! Account administration (`/admin`): searching users, and granting and revoking roles, disabling
//! and re-enabling accounts.
//!
//! [`AdminService`] holds the use cases, [`AdminPolicy`] the permission rules, [`dto`] the query
//! string. Responses reuse [`UserDto`](crate::dto::UserDto) and [`RoleDto`](crate::dto::RoleDto).
//!
//! Invariants: at least one enabled account can always manage users
//! (`ensure_someone_manages_users`, checked inside the changing transaction); an actor only
//! grants, revokes or disables what holds no permission they lack; changes need a recent sign-in;
//! nobody disables themselves.
//!
//! Not a per-entity feature: extend [`AdminService`] instead of copying it.

use domain::i18n::Message;
use domain::rbac::{Permission, RbacRepository};

use crate::error::AppError;

pub use policy::AdminPolicy;
pub use service::AdminService;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountStatus {
    Enabled,
    Disabled,
}

pub mod dto;

mod policy;
mod service;

/// Fails with a `Conflict` (`last_admin`) if no enabled account can manage users any more.
///
/// Called inside a transaction, after the change and before the commit: returning the error drops
/// the transaction, which rolls the change back. Callers take
/// [`RbacRepository::lock_role_assignments`] first, so concurrent changes cannot each believe the
/// other admin remains.
pub(crate) async fn ensure_someone_manages_users(
    store: &mut impl RbacRepository,
) -> Result<(), AppError> {
    if store
        .count_enabled_users_with(Permission::UsersManage)
        .await?
        == 0
    {
        return Err(AppError::conflict(
            "last_admin",
            Message::new("conflict-last-admin"),
        ));
    }
    Ok(())
}
