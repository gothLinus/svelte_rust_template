//! Passkeys and security keys (WebAuthn / FIDO2): public-key credentials bound to this site, used
//! to sign in without a password or as the second step after one.
//!
//! This module holds the stored [`Passkey`] and the [`WebAuthnChallenge`] of a ceremony;
//! `application::passkeys` runs the ceremonies (registration, sign-in, second factor,
//! re-authentication) and verifies responses through [`crate::security::Crypto`]. Only the public
//! key is stored. A challenge is answered at most once and only for its purpose. Storage is
//! [`PasskeyRepository`].

use std::fmt::{self, Display, Formatter};

use time::{Duration, OffsetDateTime};

use crate::{
    error::{UnknownValue, ValidationError},
    i18n::Message,
    id::Id,
    user::UserId,
};

pub use repository::PasskeyRepository;

mod repository;

pub type PasskeyId = Id<Passkey>;
pub type ChallengeId = Id<WebAuthnChallenge>;

pub const PASSKEY_CREDENTIAL_UNIQUE_CONSTRAINT: &str = "passkeys_credential_id_key";
pub const MAX_PASSKEY_NAME_LEN: usize = 100;
pub const CHALLENGE_TTL: Duration = Duration::minutes(5);
pub const MAX_CREDENTIAL_ID_LEN: usize = 1023;

/// The signature algorithms accepted, by COSE identifier. Every passkey provider supports at
/// least one: ES256 nearly everywhere, RS256 on older Windows Hello, EdDSA on some security keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PublicKeyAlgorithm {
    Es256,
    Rs256,
    EdDsa,
}

impl PublicKeyAlgorithm {
    pub const ALL: [Self; 3] = [Self::EdDsa, Self::Es256, Self::Rs256];

    pub const fn cose(self) -> i64 {
        match self {
            Self::Es256 => -7,
            Self::Rs256 => -257,
            Self::EdDsa => -8,
        }
    }

    pub fn from_cose(id: i64) -> Option<Self> {
        Self::ALL.into_iter().find(|alg| alg.cose() == id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasskeyName(String);

impl PasskeyName {
    pub fn parse(raw: &str) -> Result<Self, ValidationError> {
        let name = raw.trim();
        let len = name.chars().count();
        if len == 0 {
            Err(ValidationError::required())
        } else if len > MAX_PASSKEY_NAME_LEN {
            Err(ValidationError::new(
                "too_long",
                Message::new("validation-passkey-name-too-long").arg("max", MAX_PASSKEY_NAME_LEN),
            ))
        } else if name.chars().any(char::is_control) {
            Err(ValidationError::new(
                "invalid_characters",
                Message::new("validation-passkey-name-control-characters"),
            ))
        } else {
            Ok(Self(name.to_owned()))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for PasskeyName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Passkey {
    pub id: PasskeyId,
    pub user_id: UserId,
    pub credential_id: Vec<u8>,
    pub public_key: Vec<u8>,
    pub algorithm: PublicKeyAlgorithm,
    pub sign_count: u32,
    pub transports: Vec<String>,
    pub name: PasskeyName,
    pub created_at: OffsetDateTime,
    pub last_used_at: Option<OffsetDateTime>,
}

impl Passkey {
    /// Whether a signature counter of `new` is plausible after the stored one. A counter that does
    /// not increase means the credential may have been cloned; authenticators without a counter
    /// (synced passkeys) always report 0.
    pub fn accepts_sign_count(&self, new: u32) -> bool {
        (new == 0 && self.sign_count == 0) || new > self.sign_count
    }
}

#[derive(Debug, Clone)]
pub struct NewPasskey {
    pub id: PasskeyId,
    pub user_id: UserId,
    pub credential_id: Vec<u8>,
    pub public_key: Vec<u8>,
    pub algorithm: PublicKeyAlgorithm,
    pub sign_count: u32,
    pub transports: Vec<String>,
    pub name: PasskeyName,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChallengePurpose {
    Registration,
    Authentication,
    SecondFactor,
    Reauthentication,
}

impl ChallengePurpose {
    pub const ALL: [Self; 4] = [
        Self::Registration,
        Self::Authentication,
        Self::SecondFactor,
        Self::Reauthentication,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Registration => "registration",
            Self::Authentication => "authentication",
            Self::SecondFactor => "second_factor",
            Self::Reauthentication => "reauthentication",
        }
    }

    pub fn parse(raw: &str) -> Result<Self, UnknownValue> {
        Self::ALL
            .into_iter()
            .find(|purpose| purpose.as_str() == raw)
            .ok_or_else(|| UnknownValue::new("challenge purpose", raw))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebAuthnChallenge {
    pub id: ChallengeId,
    pub challenge: Vec<u8>,
    pub purpose: ChallengePurpose,
    pub user_id: Option<UserId>,
    pub expires_at: OffsetDateTime,
}
