//! Password hashing, bearer tokens and the other cryptographic primitives.
//!
//! - [`Argon2Hasher`] implements `domain::security::PasswordHasher` (Argon2id, PHC strings,
//!   bounded concurrency, timing-equalizing decoy hash).
//! - [`RandomTokens`] implements `TokenGenerator`: 256-bit random bearer tokens, stored only as
//!   SHA-256 digests.
//! - [`RingCrypto`] implements `Crypto` on ring: secure random values, keyed digests and
//!   authenticated encryption under the server's `SECRET_KEY`, and passkey signature
//!   verification.

pub use password::{Argon2Hasher, Argon2Param, Argon2Params, InvalidArgon2Params};
pub use primitives::{RingCrypto, SECRET_KEY_BYTES};
pub use tokens::{RandomTokens, TOKEN_BYTES};

mod password;
mod primitives;
mod tokens;
