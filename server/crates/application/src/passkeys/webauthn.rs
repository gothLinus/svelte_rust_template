//! The parts of WebAuthn verification that are parsing and comparing: client data and
//! authenticator data (Web Authentication Level 2, sections 6.1 and 7). Signatures are checked
//! through the [`Crypto`] port.
//!
//! Attestation is not requested (`"none"`), so a registration proves nothing about the
//! authenticator's make; it binds a public key to the account, which is all passkeys need.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use domain::security::Crypto;
use serde::Deserialize;

use crate::error::AppError;

const FLAG_USER_PRESENT: u8 = 0x01;
const FLAG_USER_VERIFIED: u8 = 0x04;
const FLAG_ATTESTED_CREDENTIAL: u8 = 0x40;
const MAX_FIELD_LEN: usize = 16 * 1024;

pub(crate) const TYPE_CREATE: &str = "webauthn.create";
pub(crate) const TYPE_GET: &str = "webauthn.get";

/// A binary field of a WebAuthn response, if it is not absurdly long. Every field of a response
/// goes through this before it is used.
pub(crate) fn field(bytes: &[u8]) -> Result<Vec<u8>, AppError> {
    if bytes.len() > MAX_FIELD_LEN {
        return Err(AppError::InvalidPasskey);
    }
    Ok(bytes.to_vec())
}

fn decode(field: &str) -> Result<Vec<u8>, AppError> {
    if field.len() > MAX_FIELD_LEN {
        return Err(AppError::InvalidPasskey);
    }
    URL_SAFE_NO_PAD
        .decode(field.trim_end_matches('='))
        .map_err(|_| AppError::InvalidPasskey)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClientData {
    #[serde(rename = "type")]
    kind: String,
    challenge: String,
    origin: String,
    #[serde(default)]
    cross_origin: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UserCheck {
    Presence,
    /// It verified who (UV: a PIN or biometrics): needed when the passkey is the only step.
    Verification,
}

pub(crate) struct Expected<'a> {
    pub kind: &'static str,
    pub challenge: &'a [u8],
    pub origin: &'a str,
    pub rp_id: &'a str,
    pub user_check: UserCheck,
}

pub(crate) struct AuthenticatorData<'a> {
    pub sign_count: u32,
    pub credential_id: Option<&'a [u8]>,
}

/// Checks the client data and authenticator data of a ceremony against `expected`, and returns the
/// parsed authenticator data.
///
/// Enforced: the ceremony type, the challenge, the exact origin, no cross-origin framing, the
/// SHA-256 of the relying party id, the user-present flag, and the user-verified flag when
/// `expected.user_check` is [`UserCheck::Verification`]. The signature and its counter are checked
/// by the caller.
///
/// # Errors
///
/// [`AppError::InvalidPasskey`] for any mismatch or malformed input, deliberately without saying
/// which.
pub(crate) fn verify<'a>(
    crypto: &impl Crypto,
    expected: &Expected<'_>,
    client_data_json: &[u8],
    authenticator_data: &'a [u8],
) -> Result<AuthenticatorData<'a>, AppError> {
    let client: ClientData =
        serde_json::from_slice(client_data_json).map_err(|_| AppError::InvalidPasskey)?;
    let challenge = decode(&client.challenge)?;
    if client.kind != expected.kind
        || challenge != expected.challenge
        || client.origin != expected.origin
        || client.cross_origin
    {
        return Err(AppError::InvalidPasskey);
    }

    let data = parse(authenticator_data)?;
    let rp_id_hash = &authenticator_data[..32];
    let flags = authenticator_data[32];
    if rp_id_hash != crypto.sha256(expected.rp_id.as_bytes())
        || flags & FLAG_USER_PRESENT == 0
        || (expected.user_check == UserCheck::Verification && flags & FLAG_USER_VERIFIED == 0)
    {
        return Err(AppError::InvalidPasskey);
    }
    Ok(data)
}

fn parse(bytes: &[u8]) -> Result<AuthenticatorData<'_>, AppError> {
    if bytes.len() < 37 {
        return Err(AppError::InvalidPasskey);
    }
    let flags = bytes[32];
    let sign_count = u32::from_be_bytes([bytes[33], bytes[34], bytes[35], bytes[36]]);

    let credential_id = if flags & FLAG_ATTESTED_CREDENTIAL == 0 {
        None
    } else {
        // AAGUID (16 bytes), then the credential id's length (2 bytes) and the id.
        let length = bytes
            .get(53..55)
            .map(|len| usize::from(u16::from_be_bytes([len[0], len[1]])))
            .ok_or(AppError::InvalidPasskey)?;
        Some(bytes.get(55..55 + length).ok_or(AppError::InvalidPasskey)?)
    };
    Ok(AuthenticatorData {
        sign_count,
        credential_id,
    })
}

/// The bytes an assertion signature covers: authenticator data followed by the SHA-256 of the
/// client data JSON (WebAuthn section 7.2).
pub(crate) fn signed_message(
    crypto: &impl Crypto,
    authenticator_data: &[u8],
    client_data_json: &[u8],
) -> Vec<u8> {
    let mut message = authenticator_data.to_vec();
    message.extend_from_slice(&crypto.sha256(client_data_json));
    message
}
