//! User accounts: the [`User`] entity, its value objects ([`Email`], [`Username`],
//! [`PhoneNumber`], [`NewPassword`], [`PasswordHash`]) and [`UserRepository`].
//!
//! Users are not a generic [`Resource`](crate::repository::Resource): registration, sign-in and
//! account changes have their own rules, so they are `application::auth`, `application::account`
//! and `application::admin` use cases over [`UserRepository`]. Not meant to be copied for a new
//! entity; copy [`crate::note`] for that.

use time::OffsetDateTime;

use crate::id::Id;

pub use email::{Email, MAX_EMAIL_LEN};
pub use identifier::LoginIdentifier;
pub use password::{
    MAX_PASSWORD_LEN, MIN_PASSWORD_LEN, NewPassword, PasswordHash, is_common_password,
};
pub use phone::{CallingCode, PhoneNumber};
pub use repository::UserRepository;
pub use username::{MAX_USERNAME_LEN, MIN_USERNAME_LEN, Username};

mod email;
mod identifier;
mod password;
mod phone;
mod repository;
mod username;

pub type UserId = Id<User>;

/// The name of the unique index on `lower(email)`. A write that violates it reports
/// [`StorageError::UniqueViolation`](crate::error::StorageError) with this name.
pub const EMAIL_UNIQUE_CONSTRAINT: &str = "users_email_lower_key";
pub const USERNAME_UNIQUE_CONSTRAINT: &str = "users_username_lower_key";
pub const PHONE_UNIQUE_CONSTRAINT: &str = "users_phone_key";

/// A user account. Roles and permissions are not part of the entity: they live in the RBAC tables
/// and are loaded when needed (see [`crate::rbac`]).
#[derive(Debug, Clone)]
pub struct User {
    id: UserId,
    email: Email,
    username: Username,
    phone: Option<PhoneNumber>,
    phone_verified_at: Option<OffsetDateTime>,
    /// `None` for accounts that only sign in without a password: a social account, a passkey, an
    /// emailed code.
    password_hash: Option<PasswordHash>,
    email_verified_at: Option<OffsetDateTime>,
    disabled_at: Option<OffsetDateTime>,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

pub struct UserParts {
    pub id: UserId,
    pub email: Email,
    pub username: Username,
    pub phone: Option<PhoneNumber>,
    pub phone_verified_at: Option<OffsetDateTime>,
    pub password_hash: Option<PasswordHash>,
    pub email_verified_at: Option<OffsetDateTime>,
    pub disabled_at: Option<OffsetDateTime>,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

impl User {
    pub fn from_parts(parts: UserParts) -> Self {
        Self {
            id: parts.id,
            email: parts.email,
            username: parts.username,
            phone: parts.phone,
            phone_verified_at: parts.phone_verified_at,
            password_hash: parts.password_hash,
            email_verified_at: parts.email_verified_at,
            disabled_at: parts.disabled_at,
            created_at: parts.created_at,
            updated_at: parts.updated_at,
        }
    }

    pub fn id(&self) -> UserId {
        self.id
    }

    pub fn email(&self) -> &Email {
        &self.email
    }

    pub fn username(&self) -> &Username {
        &self.username
    }

    pub fn phone(&self) -> Option<&PhoneNumber> {
        self.phone.as_ref()
    }

    pub fn phone_verified_at(&self) -> Option<OffsetDateTime> {
        self.phone_verified_at
    }

    /// The phone number, if it was confirmed with a code. Only a verified number can be used to
    /// sign in.
    pub fn verified_phone(&self) -> Option<&PhoneNumber> {
        self.phone
            .as_ref()
            .filter(|_| self.phone_verified_at.is_some())
    }

    pub fn password_hash(&self) -> Option<&PasswordHash> {
        self.password_hash.as_ref()
    }

    pub fn has_password(&self) -> bool {
        self.password_hash.is_some()
    }

    pub fn email_verified_at(&self) -> Option<OffsetDateTime> {
        self.email_verified_at
    }

    pub fn is_email_verified(&self) -> bool {
        self.email_verified_at.is_some()
    }

    pub fn disabled_at(&self) -> Option<OffsetDateTime> {
        self.disabled_at
    }

    /// A disabled user cannot sign in, and their sessions were revoked when they were disabled.
    pub fn is_disabled(&self) -> bool {
        self.disabled_at.is_some()
    }

    pub fn created_at(&self) -> OffsetDateTime {
        self.created_at
    }

    pub fn updated_at(&self) -> OffsetDateTime {
        self.updated_at
    }
}

#[derive(Debug)]
pub struct NewUser {
    pub id: UserId,
    pub email: Email,
    pub username: Username,
    pub password_hash: Option<PasswordHash>,
    pub email_verified_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Default)]
pub struct UserFilter {
    pub search: Option<String>,
}
