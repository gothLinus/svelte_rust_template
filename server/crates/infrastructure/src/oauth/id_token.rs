//! Reading an OpenID Connect ID token received straight from the token endpoint.
//!
//! The signature is not checked: OpenID Connect Core 3.1.3.7 lets the TLS connection to the
//! provider's token endpoint stand in for it, and that is the only place tokens come from here.
//! Never pass `claims_from_token_endpoint` a token that arrived any other way (from the browser,
//! in a URL fragment or a form post): anyone could have written it. It is crate-private for that
//! reason; a flow that receives tokens from the browser (One Tap, the implicit flow) needs a
//! verifier that checks the signature against the provider's JWKS.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use domain::identity::OAuthError;
use serde::Deserialize;
use time::OffsetDateTime;

const CLOCK_SKEW_SECONDS: i64 = 60;

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Audience {
    One(String),
    Many(Vec<String>),
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Flag {
    Bool(bool),
    Text(String),
}

impl Flag {
    fn is_true(&self) -> bool {
        match self {
            Self::Bool(value) => *value,
            Self::Text(value) => value == "true",
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawClaims {
    iss: String,
    aud: Audience,
    azp: Option<String>,
    exp: i64,
    sub: String,
    nonce: Option<String>,
    email: Option<String>,
    email_verified: Option<Flag>,
    name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claims {
    pub subject: String,
    pub email: Option<String>,
    pub email_verified: bool,
    pub name: Option<String>,
}

/// What a token must say to be accepted: who issued it, for which client, and the nonce of the
/// sign-in it belongs to.
pub struct Expected<'a> {
    /// Whether an `iss` claim is the provider's: a predicate, since Microsoft's issuer contains the
    /// tenant id.
    pub issuer: &'a dyn Fn(&str) -> bool,
    pub client_id: &'a str,
    pub nonce: &'a str,
}

#[derive(Debug, thiserror::Error)]
#[error("invalid ID token: {0}")]
struct InvalidIdToken(&'static str);

fn invalid(reason: &'static str) -> OAuthError {
    OAuthError::failed(InvalidIdToken(reason))
}

/// Decodes the payload of a token the provider's token endpoint just returned over TLS, and
/// checks issuer, audience, expiry and nonce. The signature is not checked.
pub(crate) fn claims_from_token_endpoint(
    token: &str,
    expected: &Expected<'_>,
) -> Result<Claims, OAuthError> {
    let mut parts = token.split('.');
    let (Some(_header), Some(payload), Some(_signature), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(invalid("not a JWT"));
    };
    let payload = URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .map_err(|_| invalid("payload is not base64url"))?;
    let claims: RawClaims = serde_json::from_slice(&payload)
        .map_err(|_| invalid("payload is not the expected JSON"))?;

    if !(expected.issuer)(&claims.iss) {
        return Err(invalid("unexpected issuer"));
    }
    // With several audiences, the token must also name us as the party it was issued to (OpenID
    // Connect Core 3.1.3.7, steps 3 to 5).
    let audience_ok = match &claims.aud {
        Audience::One(aud) => {
            aud == expected.client_id
                && claims
                    .azp
                    .as_deref()
                    .is_none_or(|azp| azp == expected.client_id)
        }
        Audience::Many(auds) => {
            auds.iter().any(|aud| aud == expected.client_id)
                && claims.azp.as_deref() == Some(expected.client_id)
        }
    };
    if !audience_ok {
        return Err(invalid("issued for another client"));
    }
    if claims.exp + CLOCK_SKEW_SECONDS < OffsetDateTime::now_utc().unix_timestamp() {
        return Err(invalid("expired"));
    }
    if claims.nonce.as_deref() != Some(expected.nonce) {
        return Err(invalid("nonce mismatch"));
    }

    Ok(Claims {
        subject: claims.sub,
        email: claims.email,
        email_verified: claims.email_verified.is_some_and(|flag| flag.is_true()),
        name: claims.name,
    })
}
