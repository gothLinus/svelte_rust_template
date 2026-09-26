use std::fmt::{self, Debug, Formatter};

use zeroize::Zeroizing;

use crate::error::UnknownValue;

/// Longer secrets are rejected before they reach a hash function, so a huge request body cannot
/// burn CPU.
pub const MAX_SECRET_BYTES: usize = 1024;

/// A password or token as the user sent it.
///
/// The memory is wiped on drop and `Debug` prints `Secret(<redacted>)`, so it cannot end up in a
/// log line by accident. Reading it takes an explicit [`Secret::expose`]; copies made from the
/// exposed `&str` are not wiped.
#[derive(Clone, Default)]
pub struct Secret(Zeroizing<String>);

impl Secret {
    pub fn new(secret: impl Into<String>) -> Self {
        Self(Zeroizing::new(secret.into()))
    }

    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn is_within_limit(&self) -> bool {
        self.0.len() <= MAX_SECRET_BYTES
    }
}

impl From<String> for Secret {
    fn from(secret: String) -> Self {
        Self::new(secret)
    }
}

impl Debug for Secret {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}

pub const TOKEN_HASH_BYTES: usize = 32;

/// The SHA-256 digest of a random bearer token (a session cookie, a link in an email).
///
/// Only the digest is stored, so a leaked database hands out no usable tokens. The tokens carry
/// 256 bits of entropy, so a fast hash is enough: there is nothing to brute-force. Equality
/// compares in constant time ([`constant_time_eq`]).
#[derive(Clone, Copy, Eq)]
pub struct TokenHash([u8; TOKEN_HASH_BYTES]);

impl PartialEq for TokenHash {
    fn eq(&self, other: &Self) -> bool {
        constant_time_eq(&self.0, &other.0)
    }
}

// Hashes the bytes that `eq` compares, so equal hashes still hash alike.
impl std::hash::Hash for TokenHash {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

/// Whether `a` and `b` are equal, taking the same time whatever bytes differ, so a guess compared
/// against a secret does not reveal how much of it was right. Unequal lengths return early; the
/// length is not secret.
pub fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let difference = a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y));
    // `black_box` keeps the optimizer from turning the fold into an early exit.
    std::hint::black_box(difference) == 0
}

impl TokenHash {
    pub const fn new(digest: [u8; TOKEN_HASH_BYTES]) -> Self {
        Self(digest)
    }

    pub fn from_slice(bytes: &[u8]) -> Result<Self, UnknownValue> {
        bytes
            .try_into()
            .map(Self)
            .map_err(|_| UnknownValue::new("token hash length", bytes.len().to_string()))
    }

    pub fn as_bytes(&self) -> &[u8; TOKEN_HASH_BYTES] {
        &self.0
    }
}

impl Debug for TokenHash {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("TokenHash(<redacted>)")
    }
}
