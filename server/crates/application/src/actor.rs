use domain::{
    i18n::Message,
    rbac::{Permission, PermissionSet},
    session::SessionId,
    user::UserId,
};

use crate::error::AppError;

/// The authenticated user behind a request, with the permissions they hold right now.
///
/// Permissions are loaded from the database on every request, so a role change takes effect on
/// the user's very next request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Actor {
    pub user_id: UserId,
    pub session_id: SessionId,
    pub permissions: PermissionSet,
    pub email_verified: bool,
    /// The session signed in or re-authenticated within
    /// [`REAUTH_WINDOW`](domain::session::REAUTH_WINDOW).
    pub recently_authenticated: bool,
}

impl Actor {
    pub fn has(&self, permission: Permission) -> bool {
        self.permissions.contains(permission)
    }

    pub fn require(&self, permission: Permission) -> Result<(), AppError> {
        if self.has(permission) {
            Ok(())
        } else {
            Err(AppError::Forbidden)
        }
    }

    /// `Err(ReauthRequired)` unless the session proved who is behind it recently. Every change that
    /// adds or removes a way into the account, moves its address or number, or deletes it needs
    /// this: a stolen session alone must not be enough to lock the owner out.
    pub fn require_recent_authentication(&self) -> Result<(), AppError> {
        if self.recently_authenticated {
            Ok(())
        } else {
            Err(AppError::ReauthRequired)
        }
    }

    /// `Err` unless the actor's email address is verified. Adding a way into the account (a
    /// passkey, an authenticator app, a provider account, a phone number) needs it: until the
    /// address is proven, the account may belong to someone who registered another person's
    /// address, and whatever they attach would survive that person's recovery.
    pub fn require_verified_email(&self) -> Result<(), AppError> {
        if self.email_verified {
            Ok(())
        } else {
            Err(AppError::conflict(
                "email_unverified",
                Message::new("conflict-email-unverified"),
            ))
        }
    }
}
