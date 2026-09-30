use std::{
    borrow::Cow,
    fmt::{self, Display, Formatter},
};

use crate::{
    error::ValidationError,
    i18n::Message,
    rbac::{Permission, PermissionSet},
};

pub const MAX_ROLE_NAME_LEN: usize = 50;

/// What the default role, [`RoleName::USER`], grants: every new account holds these.
///
/// The migrations seed the same list and a Postgres test compares the two. A feature whose
/// permissions everyone gets adds them here and grants them to `user` in its own migration.
pub const DEFAULT_USER_PERMISSIONS: &[Permission] = &[
    Permission::NotesRead,
    Permission::NotesWrite,
    Permission::FilesRead,
    Permission::FilesWrite,
];

pub fn default_user_permissions() -> PermissionSet {
    DEFAULT_USER_PERMISSIONS.iter().copied().collect()
}

/// The name of a role: lowercase ASCII letters, digits, `_` and `-`, starting with a letter.
///
/// Roles are reference data keyed by this name rather than a UUID, so migrations can seed and
/// reference them without hard-coding ids.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RoleName(Cow<'static, str>);

impl RoleName {
    pub const ADMIN: Self = Self(Cow::Borrowed("admin"));
    pub const USER: Self = Self(Cow::Borrowed("user"));

    pub fn parse(raw: &str) -> Result<Self, ValidationError> {
        let valid = raw.len() <= MAX_ROLE_NAME_LEN
            && raw.starts_with(|c: char| c.is_ascii_lowercase())
            && raw
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-');

        if valid {
            Ok(Self(Cow::Owned(raw.to_owned())))
        } else {
            Err(ValidationError::new(
                "invalid_role",
                Message::new("validation-invalid-role"),
            ))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for RoleName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Role {
    pub name: RoleName,
    pub description: String,
    pub permissions: PermissionSet,
}
