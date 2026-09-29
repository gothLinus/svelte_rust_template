//! Short numeric codes sent by email or text message: signing in without a password, and proving
//! a phone number belongs to the user.
//!
//! Six digits are guessable where a link token is not, so a code only works for a few minutes and
//! for [`MAX_CODE_ATTEMPTS`] wrong guesses; the rate limits on top keep an attacker far below
//! that. Like link tokens, only a digest is stored.
//!
//! `application::passwordless` issues login codes, `application::account` phone verification
//! codes and `application::auth` re-authentication codes; each user holds at most one live code
//! per [`CodePurpose`]. Storage is [`OneTimeCodeRepository`].

use std::fmt::{self, Display, Formatter};

use time::{Duration, OffsetDateTime};

use crate::{
    error::UnknownValue,
    secret::TokenHash,
    user::{PhoneNumber, UserId},
};

pub use repository::OneTimeCodeRepository;

mod repository;

pub const CODE_DIGITS: u32 = 6;
pub const MAX_CODE_ATTEMPTS: u32 = 5;
pub const CODE_TTL: Duration = Duration::minutes(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CodePurpose {
    Login,
    PhoneVerification,
    Reauthentication,
}

impl CodePurpose {
    pub const ALL: [Self; 3] = [Self::Login, Self::PhoneVerification, Self::Reauthentication];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Login => "login",
            Self::PhoneVerification => "phone_verification",
            Self::Reauthentication => "reauthentication",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, UnknownValue> {
        Self::ALL
            .into_iter()
            .find(|purpose| purpose.as_str() == raw)
            .ok_or_else(|| UnknownValue::new("code purpose", raw))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CodeChannel {
    Email,
    Sms,
    Whatsapp,
}

impl CodeChannel {
    pub const ALL: [Self; 3] = [Self::Email, Self::Sms, Self::Whatsapp];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Email => "email",
            Self::Sms => "sms",
            Self::Whatsapp => "whatsapp",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, UnknownValue> {
        Self::ALL
            .into_iter()
            .find(|channel| channel.as_str() == raw)
            .ok_or_else(|| UnknownValue::new("delivery channel", raw))
    }
}

impl Display for CodeChannel {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A live code. Each user has at most one per purpose; sending a new one replaces it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OneTimeCode {
    pub user_id: UserId,
    pub purpose: CodePurpose,
    pub channel: CodeChannel,
    pub code_hash: TokenHash,
    pub target: Option<PhoneNumber>,
    pub attempts: u32,
    pub expires_at: OffsetDateTime,
}

impl OneTimeCode {
    pub fn issue(
        user_id: UserId,
        purpose: CodePurpose,
        channel: CodeChannel,
        code_hash: TokenHash,
        now: OffsetDateTime,
    ) -> Self {
        Self {
            user_id,
            purpose,
            channel,
            code_hash,
            target: None,
            attempts: 0,
            expires_at: now.saturating_add(CODE_TTL),
        }
    }

    pub fn is_live(&self, now: OffsetDateTime) -> bool {
        now < self.expires_at && self.attempts < MAX_CODE_ATTEMPTS
    }
}
