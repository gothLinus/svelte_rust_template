use domain::{
    mfa::{TOTP_DIGITS, TOTP_PERIOD_SECONDS, TOTP_SKEW_STEPS, hotp_code, totp_step},
    secret::constant_time_eq,
    security::Crypto,
};
use time::OffsetDateTime;

use crate::codes::normalize;

/// The time step `code` is valid for, within the allowed clock skew around `now`
/// (`TOTP_SKEW_STEPS` either side), or `None`.
///
/// This only tells that the code matches a step. The caller must then record the step
/// (`MfaRepository::use_totp_step`) so an accepted code cannot be used again (RFC 6238 section
/// 5.2). Each comparison is constant-time.
pub fn matching_step(
    crypto: &impl Crypto,
    secret: &[u8],
    code: &str,
    now: OffsetDateTime,
) -> Option<i64> {
    let code = normalize(code);
    if code.len() != TOTP_DIGITS as usize {
        return None;
    }
    let current = totp_step(now);
    (-TOTP_SKEW_STEPS..=TOTP_SKEW_STEPS)
        .map(|offset| current + offset)
        .find(|step| {
            let mac = crypto.hmac_sha1(secret, &step.to_be_bytes());
            constant_time_eq(hotp_code(&mac).as_bytes(), code.as_bytes())
        })
}

/// What an authenticator app's sealed secret is bound to: its owner, so a sealed value copied to
/// another user's row does not open.
pub(crate) fn context(user: domain::user::UserId) -> Vec<u8> {
    format!("totp:{user}").into_bytes()
}

pub fn base32(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 32] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut out = String::with_capacity(bytes.len().div_ceil(5) * 8);
    let mut buffer: u32 = 0;
    let mut bits = 0;
    for &byte in bytes {
        buffer = (buffer << 8) | u32::from(byte);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(char::from(ALPHABET[((buffer >> bits) & 31) as usize]));
        }
    }
    if bits > 0 {
        out.push(char::from(ALPHABET[((buffer << (5 - bits)) & 31) as usize]));
    }
    out
}

pub(crate) fn otpauth_uri(issuer: &str, account: &str, secret: &str) -> String {
    format!(
        "otpauth://totp/{}:{}?secret={secret}&issuer={}&algorithm=SHA1&digits={TOTP_DIGITS}&period={TOTP_PERIOD_SECONDS}",
        percent_encode(issuer),
        percent_encode(account),
        percent_encode(issuer),
    )
}

fn percent_encode(value: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push('%');
            out.push(char::from(HEX[usize::from(byte >> 4)]));
            out.push(char::from(HEX[usize::from(byte & 0x0f)]));
        }
    }
    out
}
