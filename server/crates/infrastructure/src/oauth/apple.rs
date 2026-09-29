use std::fmt::{self, Debug, Formatter};

use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use domain::identity::OAuthError;
use ring::{
    rand::SystemRandom,
    signature::{ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair},
};
use thiserror::Error;
use time::OffsetDateTime;

#[derive(Debug, Error)]
#[error("OAUTH_APPLE_PRIVATE_KEY must be the PKCS#8 .p8 key from Apple (PEM)")]
pub struct InvalidAppleKey;

#[derive(Debug, Error)]
#[error("signing the Apple client secret failed")]
struct SigningFailed;

#[derive(Clone)]
pub struct AppleKey {
    pub team_id: String,
    pub key_id: String,
    pkcs8: Vec<u8>,
}

impl Debug for AppleKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppleKey")
            .field("team_id", &self.team_id)
            .field("key_id", &self.key_id)
            .field("pkcs8", &"<redacted>")
            .finish()
    }
}

impl AppleKey {
    /// Parses the PEM (a literal `\n` counts as a line break, so it fits in one line of `.env`) and
    /// checks that it is a P-256 signing key.
    pub fn new(team_id: String, key_id: String, pem: &str) -> Result<Self, InvalidAppleKey> {
        let body: String = pem
            .replace("\\n", "\n")
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with("-----"))
            .collect();
        let pkcs8 = STANDARD.decode(body).map_err(|_| InvalidAppleKey)?;
        EcdsaKeyPair::from_pkcs8(
            &ECDSA_P256_SHA256_FIXED_SIGNING,
            &pkcs8,
            &SystemRandom::new(),
        )
        .map_err(|_| InvalidAppleKey)?;
        Ok(Self {
            team_id,
            key_id,
            pkcs8,
        })
    }

    pub(crate) fn client_secret(&self, client_id: &str) -> Result<String, OAuthError> {
        let rng = SystemRandom::new();
        let key = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &self.pkcs8, &rng)
            .map_err(|_| OAuthError::failed(SigningFailed))?;

        let now = OffsetDateTime::now_utc().unix_timestamp();
        let header = serde_json::json!({ "alg": "ES256", "kid": self.key_id });
        let claims = serde_json::json!({
            "iss": self.team_id,
            "iat": now,
            "exp": now + 300,
            "aud": "https://appleid.apple.com",
            "sub": client_id,
        });
        let signing_input = format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(header.to_string()),
            URL_SAFE_NO_PAD.encode(claims.to_string())
        );
        let signature = key
            .sign(&rng, signing_input.as_bytes())
            .map_err(|_| OAuthError::failed(SigningFailed))?;
        Ok(format!(
            "{signing_input}.{}",
            URL_SAFE_NO_PAD.encode(signature.as_ref())
        ))
    }
}
