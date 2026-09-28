use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use domain::{
    secret::{Secret, TokenHash},
    security::{TokenError, TokenGenerator},
};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

pub const TOKEN_BYTES: usize = 32;

/// Tokens from the operating system's random number generator, base64url-encoded (43 characters,
/// safe in cookies and URLs), stored as their SHA-256 digest.
#[derive(Debug, Clone, Copy, Default)]
pub struct RandomTokens;

impl TokenGenerator for RandomTokens {
    fn generate(&self) -> Result<Secret, TokenError> {
        let mut bytes = Zeroizing::new([0u8; TOKEN_BYTES]);
        getrandom::fill(bytes.as_mut()).map_err(|err| TokenError::new(RandomError(err)))?;
        Ok(Secret::new(URL_SAFE_NO_PAD.encode(bytes.as_ref())))
    }

    fn digest(&self, token: &Secret) -> TokenHash {
        TokenHash::new(Sha256::digest(token.expose().as_bytes()).into())
    }
}

#[derive(Debug, thiserror::Error)]
#[error("the operating system's random number generator failed: {0}")]
struct RandomError(getrandom::Error);
