//! DTOs shared by several features. Feature-specific DTOs live next to their service, such as
//! `notes::dto`.
//!
//! DTOs are plain Rust: ids are [`Uuid`]s, timestamps [`OffsetDateTime`]s and secrets
//! [`SecretInput`]s. The HTTP layer (`api::wire`) converts them to and from the Protocol Buffers
//! messages in `/proto`.
//!
//! - Request DTOs carry the raw, unvalidated input; the service validates it through the
//!   domain's value objects and reports every bad field at once
//!   ([`ValidationErrors`](crate::ValidationErrors)).
//! - Response DTOs are built from domain entities and expose no internal state, such as password
//!   hashes or token digests.
//! - Lists are returned as [`PageDto`](crate::pagination::PageDto).

use std::fmt::{self, Debug, Formatter};

use domain::{
    rbac::{Permission, PermissionSet, Role, RoleName},
    secret::Secret,
    session::Session,
    user::User,
};
use time::OffsetDateTime;
use uuid::Uuid;

/// A password or token from a request body, moved straight into a [`Secret`] so it is redacted
/// from `Debug` and wiped from memory once the request is done.
#[derive(Clone, Default)]
pub struct SecretInput(pub Secret);

impl From<String> for SecretInput {
    fn from(secret: String) -> Self {
        Self(Secret::new(secret))
    }
}

impl Debug for SecretInput {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

macro_rules! permission_names {
    ($($variant:ident => $name:literal, $description:literal;)*) => {
        /// Something a user may be allowed to do, named `resource:action`. The variants come from
        /// [`domain::for_each_permission!`], like `domain::rbac::Permission`'s.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum PermissionName {
            $(
                #[doc = $description]
                $variant,
            )*
        }

        impl From<Permission> for PermissionName {
            fn from(permission: Permission) -> Self {
                match permission {
                    $(Permission::$variant => Self::$variant,)*
                }
            }
        }

        impl From<PermissionName> for Permission {
            fn from(name: PermissionName) -> Self {
                match name {
                    $(PermissionName::$variant => Self::$variant,)*
                }
            }
        }
    };
}

domain::for_each_permission!(permission_names);

pub fn permission_names(permissions: PermissionSet) -> Vec<PermissionName> {
    permissions.iter().map(PermissionName::from).collect()
}

#[derive(Debug, Clone)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "flat booleans are what the frontend wants to read; this is a wire format"
)]
pub struct UserDto {
    pub id: Uuid,
    pub email: String,
    pub username: String,
    pub phone: Option<String>,
    pub phone_verified: bool,
    pub email_verified: bool,
    pub has_password: bool,
    pub disabled: bool,
    pub roles: Vec<String>,
    pub created_at: OffsetDateTime,
}

impl UserDto {
    pub fn new(user: &User, roles: &[RoleName]) -> Self {
        Self {
            id: user.id().as_uuid(),
            email: user.email().to_string(),
            username: user.username().to_string(),
            phone: user.phone().map(ToString::to_string),
            phone_verified: user.verified_phone().is_some(),
            email_verified: user.is_email_verified(),
            has_password: user.has_password(),
            disabled: user.is_disabled(),
            roles: roles.iter().map(ToString::to_string).collect(),
            created_at: user.created_at(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct MeDto {
    pub user: UserDto,
    /// What the user may do. For showing and hiding UI only; the server checks again.
    pub permissions: Vec<PermissionName>,
}

#[derive(Debug, Clone)]
pub struct RoleDto {
    pub name: String,
    pub description: String,
    pub permissions: Vec<PermissionName>,
}

impl From<Role> for RoleDto {
    fn from(role: Role) -> Self {
        Self {
            name: role.name.to_string(),
            description: role.description,
            permissions: permission_names(role.permissions),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SessionDto {
    pub id: Uuid,
    pub current: bool,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
    pub created_at: OffsetDateTime,
    pub last_seen_at: OffsetDateTime,
    pub expires_at: OffsetDateTime,
}

impl SessionDto {
    pub fn new(session: &Session, current: bool, policy: &domain::session::SessionPolicy) -> Self {
        Self {
            id: session.id().as_uuid(),
            current,
            ip: session.client().ip.map(|ip| ip.to_string()),
            user_agent: session.client().user_agent.clone(),
            created_at: session.created_at(),
            last_seen_at: session.last_seen_at(),
            expires_at: session.idle_expires_at(policy),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MfaMethod {
    Totp,
    Passkey,
    RecoveryCode,
}

/// The first sign-in step succeeded and a second one is needed. The server set a short-lived
/// cookie that identifies the attempt. Maps to the `MfaChallenge` message.
#[derive(Debug, Clone)]
pub struct MfaChallengeDto {
    pub methods: Vec<MfaMethod>,
}
