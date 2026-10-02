use domain::{
    i18n::Message,
    rbac::{Permission, PermissionSet},
    user::UserId,
};

use crate::{actor::Actor, error::AppError};

/// Rules for the admin use cases. Beyond the permission checks, it stops admins from locking
/// themselves out and from reaching above their own station: an actor only grants or revokes
/// roles, and only disables accounts, whose permissions they hold themselves, so a `moderator`
/// role with `users:manage` cannot mint admins or disable them. The "at least one user manager
/// remains" rule needs the database and lives in `super::ensure_someone_manages_users`, which the
/// service calls in the same transaction.
///
/// Every change here needs a recent sign-in or step-up: a stolen session alone must not hand out
/// access.
///
/// Unlike [`Policy`](crate::policy::Policy), these are plain checks on the actor and the target,
/// each returning the `AppError` to report.
pub struct AdminPolicy;

impl AdminPolicy {
    pub fn can_view_users(actor: &Actor) -> Result<(), AppError> {
        actor.require(Permission::UsersRead)
    }

    pub fn can_change_roles(actor: &Actor) -> Result<(), AppError> {
        actor.require(Permission::UsersManage)?;
        actor.require_recent_authentication()
    }

    /// Whether the actor may grant or revoke a role with `permissions`: only if they hold all of
    /// them.
    pub fn can_assign(actor: &Actor, permissions: PermissionSet) -> Result<(), AppError> {
        if actor.permissions.contains_all(permissions) {
            Ok(())
        } else {
            Err(AppError::Forbidden)
        }
    }

    pub fn can_set_disabled(actor: &Actor, target: UserId) -> Result<(), AppError> {
        actor.require(Permission::UsersManage)?;
        actor.require_recent_authentication()?;
        if target == actor.user_id {
            return Err(AppError::conflict(
                "cannot_disable_self",
                Message::new("conflict-cannot-disable-self"),
            ));
        }
        Ok(())
    }

    pub fn can_revoke_sessions(actor: &Actor) -> Result<(), AppError> {
        actor.require(Permission::UsersManage)?;
        actor.require_recent_authentication()
    }

    /// Whether the actor may disable or re-enable an account holding `permissions`, or sign it
    /// out: only if they hold all of them.
    pub fn can_set_disabled_holder_of(
        actor: &Actor,
        permissions: PermissionSet,
    ) -> Result<(), AppError> {
        Self::can_assign(actor, permissions)
    }
}
