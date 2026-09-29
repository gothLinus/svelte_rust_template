//! The signed-in user's own account (`/me`): profile, email and password changes, phone number,
//! sessions, the security overview, the personal data export and deletion.
//!
//! [`AccountService`] holds the use cases, [`dto`] their bodies. Signing in and the second-factor,
//! passkey and provider settings live in their own features.
//!
//! Invariants: a change that adds or removes a way into the account, moves its address or number,
//! exports it or deletes it needs a recent sign-in
//! ([`Actor::require_recent_authentication`](crate::actor::Actor::require_recent_authentication));
//! adding a phone number also needs a verified email; email and password changes go
//! through a mailed link, never the old password; deleting an account cannot remove the last user
//! manager.
//!
//! Not a per-entity feature: extend [`AccountService`] instead of copying it.

use domain::{
    error::StorageError,
    rbac::{RbacRepository, RoleName},
    user::User,
};

use crate::dto::{MeDto, UserDto, permission_names};

pub use service::AccountService;

pub mod dto;

mod service;

pub(crate) async fn load_me(
    store: &mut impl RbacRepository,
    user: &User,
) -> Result<MeDto, StorageError> {
    let roles: Vec<RoleName> = store
        .roles_of_users(&[user.id()])
        .await?
        .into_iter()
        .map(|(_, role)| role)
        .collect();
    let permissions = store.permissions_of_user(user.id()).await?;

    Ok(MeDto {
        user: UserDto::new(user, &roles),
        permissions: permission_names(permissions),
    })
}
