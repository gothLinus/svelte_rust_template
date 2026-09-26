//! Ports for password hashing, random tokens and the other cryptography the use cases need. The
//! implementations (Argon2id, the OS random number generator, SHA-256, ring) live in
//! `infrastructure`.

use std::{error::Error as StdError, future::Future};

use thiserror::Error;
use zeroize::Zeroizing;

use crate::{
    passkey::PublicKeyAlgorithm,
    secret::{Secret, TokenHash},
    user::PasswordHash,
};

#[derive(Debug, Error)]
pub enum HashError {
    /// Every hashing slot is taken and the queue behind them is full, so a burst of sign-ins is
    /// answered at once instead of waiting until the request times out.
    #[error("too many passwords are being hashed")]
    Busy,
    #[error("password hashing failed")]
    Failed(#[source] Box<dyn StdError + Send + Sync>),
}

impl HashError {
    pub fn new(err: impl StdError + Send + Sync + 'static) -> Self {
        Self::Failed(Box::new(err))
    }
}

/// Hashes and verifies passwords. Implementations must be slow on purpose (a memory-hard KDF)
/// and must not block the async runtime while doing so.
///
/// Hashes are self-describing strings ([`PasswordHash`], PHC format), so a hash made with older
/// parameters still verifies; [`PasswordHasher::needs_rehash`] tells the caller when to replace
/// it.
pub trait PasswordHasher: Send + Sync + 'static {
    /// A fresh hash of `password` with a new random salt.
    ///
    /// # Errors
    ///
    /// [`HashError::Busy`] if the hasher is saturated.
    fn hash(
        &self,
        password: &Secret,
    ) -> impl Future<Output = Result<PasswordHash, HashError>> + Send;

    /// Checks `password` against `hash`.
    ///
    /// With `None` (no such account) it still does the full amount of work against a decoy hash and
    /// reports a mismatch, so a login for an unknown email takes as long as one with a wrong
    /// password.
    ///
    /// `Ok(false)` is a mismatch. An error means it could not be decided, and must not be treated
    /// as either outcome.
    fn verify(
        &self,
        password: &Secret,
        hash: Option<&PasswordHash>,
    ) -> impl Future<Output = Result<bool, HashError>> + Send;

    fn needs_rehash(&self, hash: &PasswordHash) -> bool;
}

#[derive(Debug, Error)]
#[error("generating a random token failed")]
pub struct TokenError(#[source] Box<dyn StdError + Send + Sync>);

impl TokenError {
    pub fn new(err: impl StdError + Send + Sync + 'static) -> Self {
        Self(Box::new(err))
    }
}

/// Creates unguessable bearer tokens (session cookies, email links) and the digests they are
/// stored under.
///
/// Only the digest is persisted ([`TokenHash`]). Because it is a fast hash, tokens must come
/// from a cryptographically secure generator with enough entropy that guessing is infeasible;
/// production uses 256 bits from the operating system.
pub trait TokenGenerator: Send + Sync + 'static {
    fn generate(&self) -> Result<Secret, TokenError>;

    fn digest(&self, token: &Secret) -> TokenHash;
}

#[derive(Debug, Error)]
#[error("a cryptographic operation failed")]
pub struct CryptoError(#[source] Box<dyn StdError + Send + Sync>);

impl CryptoError {
    pub fn new(err: impl StdError + Send + Sync + 'static) -> Self {
        Self(Box::new(err))
    }
}

/// The primitives the second-factor and passkey use cases build on. Everything here is fast and
/// synchronous.
///
/// Implementations must use a cryptographically secure random source and keep secret material
/// out of logs and `Debug` output.
pub trait Crypto: Send + Sync + 'static {
    fn random_bytes(&self, len: usize) -> Result<Vec<u8>, TokenError>;

    fn random_digits(&self, digits: u32) -> Result<Secret, TokenError>;

    fn sha256(&self, data: &[u8]) -> [u8; 32];

    fn hmac_sha1(&self, key: &[u8], message: &[u8]) -> [u8; 20];

    fn keyed_digest(&self, purpose: &str, data: &[u8]) -> [u8; 32];

    /// [`Crypto::keyed_digest`] under the previous key while one is configured for a key rotation
    /// (`SECRET_KEY_PREVIOUS`), so long-lived digests made before it (recovery codes) keep
    /// matching.
    fn previous_keyed_digest(&self, purpose: &str, data: &[u8]) -> Option<[u8; 32]>;

    /// Encrypts with the server's key (authenticated encryption), for secrets that must be read
    /// back, such as authenticator app seeds. `context` (the owner's id, say) is authenticated but
    /// not stored: the sealed value only opens with the same context, so it cannot be copied to
    /// another user's row.
    fn seal(&self, plaintext: &[u8], context: &[u8]) -> Result<Vec<u8>, CryptoError>;

    fn open(&self, sealed: &[u8], context: &[u8]) -> Result<Zeroizing<Vec<u8>>, CryptoError>;

    fn is_valid_public_key(&self, algorithm: PublicKeyAlgorithm, public_key: &[u8]) -> bool;

    fn verify_signature(
        &self,
        algorithm: PublicKeyAlgorithm,
        public_key: &[u8],
        message: &[u8],
        signature: &[u8],
    ) -> bool;
}
