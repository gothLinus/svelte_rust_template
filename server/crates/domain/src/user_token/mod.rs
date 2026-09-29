//! Single-use tokens sent by email: address verification, password reset, confirming a new
//! address and magic sign-in links.
//!
//! Like session tokens, only their SHA-256 digest is stored. A token is deleted the moment it is
//! used, and each user has at most one live token per purpose, so requesting a new link
//! invalidates the previous one.
//!
//! `application::auth` and `application::account` issue and consume the verification, reset,
//! email-change and cancel tokens, `application::passwordless` the magic links. Storage is
//! [`UserTokenRepository`]; the lifetimes are in [`TokenPolicy`].

use std::fmt::{self, Display, Formatter};

use time::{Duration, OffsetDateTime};

use crate::{
    error::UnknownValue,
    secret::TokenHash,
    user::{Email, UserId},
};

pub use repository::UserTokenRepository;

mod repository;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TokenPurpose {
    EmailVerification,
    PasswordReset,
    EmailChange,
    MagicLink,
    EmailChangeCancel,
}

impl TokenPurpose {
    pub const ALL: [Self; 5] = [
        Self::EmailVerification,
        Self::PasswordReset,
        Self::EmailChange,
        Self::MagicLink,
        Self::EmailChangeCancel,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::EmailVerification => "email_verification",
            Self::PasswordReset => "password_reset",
            Self::EmailChange => "email_change",
            Self::MagicLink => "magic_link",
            Self::EmailChangeCancel => "email_change_cancel",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, UnknownValue> {
        Self::ALL
            .into_iter()
            .find(|purpose| purpose.as_str() == raw)
            .ok_or_else(|| UnknownValue::new("token purpose", raw))
    }
}

impl Display for TokenPurpose {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TokenPolicy {
    pub email_verification_ttl: Duration,
    pub password_reset_ttl: Duration,
    pub email_change_ttl: Duration,
    pub magic_link_ttl: Duration,
}

impl Default for TokenPolicy {
    fn default() -> Self {
        Self {
            email_verification_ttl: Duration::days(1),
            password_reset_ttl: Duration::minutes(30),
            email_change_ttl: Duration::days(1),
            magic_link_ttl: Duration::minutes(15),
        }
    }
}

impl TokenPolicy {
    pub fn ttl(&self, purpose: TokenPurpose) -> Duration {
        match purpose {
            TokenPurpose::EmailVerification => self.email_verification_ttl,
            TokenPurpose::PasswordReset => self.password_reset_ttl,
            TokenPurpose::EmailChange | TokenPurpose::EmailChangeCancel => self.email_change_ttl,
            TokenPurpose::MagicLink => self.magic_link_ttl,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserToken {
    pub user_id: UserId,
    pub purpose: TokenPurpose,
    pub token_hash: TokenHash,
    /// The address the token is about: the new one for [`TokenPurpose::EmailChange`], the one to
    /// restore for [`TokenPurpose::EmailChangeCancel`].
    pub email: Option<Email>,
    pub expires_at: OffsetDateTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsumedToken {
    pub user_id: UserId,
    pub email: Option<Email>,
}

impl UserToken {
    pub fn issue(
        user_id: UserId,
        purpose: TokenPurpose,
        token_hash: TokenHash,
        now: OffsetDateTime,
        policy: &TokenPolicy,
    ) -> Self {
        Self {
            user_id,
            purpose,
            token_hash,
            email: None,
            expires_at: now.saturating_add(policy.ttl(purpose)),
        }
    }

    pub fn email_change(
        user_id: UserId,
        new_email: Email,
        token_hash: TokenHash,
        now: OffsetDateTime,
        policy: &TokenPolicy,
    ) -> Self {
        Self {
            email: Some(new_email),
            ..Self::issue(user_id, TokenPurpose::EmailChange, token_hash, now, policy)
        }
    }

    /// A token for the old address that cancels a pending change to another one, or undoes it by
    /// restoring `old_email`.
    pub fn email_change_cancel(
        user_id: UserId,
        old_email: Email,
        token_hash: TokenHash,
        now: OffsetDateTime,
        policy: &TokenPolicy,
    ) -> Self {
        Self {
            email: Some(old_email),
            ..Self::issue(
                user_id,
                TokenPurpose::EmailChangeCancel,
                token_hash,
                now,
                policy,
            )
        }
    }
}
