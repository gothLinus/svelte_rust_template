use std::{
    fmt::{self, Debug, Display, Formatter},
    str::FromStr,
};

use crate::{error::ValidationError, i18n::Message};

/// The one list of permissions: variant, stable name (`<resource>:<action>`, used in the
/// database and the API) and description.
///
/// Everything that enumerates permissions expands this list through a callback macro:
/// [`Permission`], the API's `PermissionName` and the route guards' markers. Adding a
/// permission is one line here plus a migration that inserts it and grants it to `admin`; a
/// test checks the database against this list.
///
/// ```
/// macro_rules! names {
///     ($($variant:ident => $name:literal, $description:literal;)*) => {
///         const NAMES: &[&str] = &[$($name),*];
///     };
/// }
/// domain::for_each_permission!(names);
/// assert!(NAMES.contains(&"notes:read"));
/// ```
#[macro_export]
macro_rules! for_each_permission {
    ($callback:ident) => {
        $callback! {
            NotesRead => "notes:read", "Read your own notes";
            NotesWrite => "notes:write", "Create, edit and delete your own notes";
            NotesManage => "notes:manage", "Read, edit and delete anyone's notes";
            UsersRead => "users:read", "List and view user accounts";
            UsersManage => "users:manage", "Assign roles, disable and enable accounts";
            AuditRead => "audit:read", "View the audit log of every account";
        }
    };
}

macro_rules! permissions {
    ($($variant:ident => $name:literal, $description:literal;)*) => {
        /// Something a user may be allowed to do. Authorization always asks for a permission, never
        /// for a role, so roles stay pure data. The variants come from
        /// [`for_each_permission!`](crate::for_each_permission).
        #[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
        pub enum Permission {
            $(
                #[doc = $description]
                $variant,
            )*
        }

        impl Permission {
            pub const ALL: [Self; [$($name),*].len()] = [$(Self::$variant),*];

            /// The stable name used in the database and the API, `<resource>:<action>`.
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $name,)*
                }
            }

            pub const fn description(self) -> &'static str {
                match self {
                    $(Self::$variant => $description,)*
                }
            }
        }
    };
}

for_each_permission!(permissions);

impl Permission {
    pub fn parse(raw: &str) -> Result<Self, ValidationError> {
        Self::ALL
            .into_iter()
            .find(|permission| permission.as_str() == raw)
            .ok_or_else(|| {
                ValidationError::new(
                    "unknown_permission",
                    Message::new("validation-unknown-permission").arg("permission", raw),
                )
            })
    }

    const fn bit(self) -> u128 {
        1 << self as u32
    }
}

impl Debug for Permission {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Display for Permission {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Permission {
    type Err = ValidationError;

    fn from_str(raw: &str) -> Result<Self, Self::Err> {
        Self::parse(raw)
    }
}

/// A set of permissions, stored as a bit set so checking one is a single AND. Room for 128; the
/// assertion below stops the build before a 129th would alias the first.
#[derive(Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct PermissionSet(u128);

const _: () = assert!(
    Permission::ALL.len() <= u128::BITS as usize,
    "PermissionSet has one bit per permission; widen it before adding more"
);

impl PermissionSet {
    pub const fn empty() -> Self {
        Self(0)
    }

    pub fn all() -> Self {
        Permission::ALL.into_iter().collect()
    }

    pub const fn contains(self, permission: Permission) -> bool {
        self.0 & permission.bit() != 0
    }

    pub const fn contains_all(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    pub fn insert(&mut self, permission: Permission) {
        self.0 |= permission.bit();
    }

    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn iter(self) -> impl Iterator<Item = Permission> {
        Permission::ALL
            .into_iter()
            .filter(move |permission| self.contains(*permission))
    }
}

impl FromIterator<Permission> for PermissionSet {
    fn from_iter<I: IntoIterator<Item = Permission>>(iter: I) -> Self {
        let mut set = Self::empty();
        for permission in iter {
            set.insert(permission);
        }
        set
    }
}

impl Debug for PermissionSet {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}
