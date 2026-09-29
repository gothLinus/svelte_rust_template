//! Two-step sign-in: authenticator apps (TOTP), recovery codes, and the challenge between the
//! first and the second step.
//!
//! It is on for a user with a confirmed authenticator app or at least one passkey. Signing in
//! with a password, an emailed or texted code or a social account then also asks for a code from
//! the app, a passkey, or a recovery code. A passkey alone already proves two factors (the device
//! and its unlock), so it skips the second step.
//!
//! `application::mfa` sets up and checks authenticator apps and recovery codes;
//! `application::auth` stores an [`MfaChallenge`] when a first step finds a second factor, and
//! only finishing that challenge starts the session. The authenticator secret is stored sealed
//! with the server's key, and recovery codes only as digests. Storage is [`MfaRepository`].

use time::{Duration, OffsetDateTime};

use crate::{secret::TokenHash, user::UserId};

pub use repository::MfaRepository;

mod repository;

pub const MFA_CHALLENGE_TTL: Duration = Duration::minutes(5);
pub const MAX_MFA_ATTEMPTS: u32 = 5;
pub const RECOVERY_CODE_COUNT: usize = 10;

/// RFC 6238 parameters every authenticator app supports: HMAC-SHA1, 30-second steps, six digits.
pub const TOTP_PERIOD_SECONDS: i64 = 30;
pub const TOTP_DIGITS: u32 = 6;
pub const TOTP_SKEW_STEPS: i64 = 1;
pub const TOTP_SECRET_BYTES: usize = 20;

pub fn totp_step(at: OffsetDateTime) -> i64 {
    at.unix_timestamp().div_euclid(TOTP_PERIOD_SECONDS)
}

/// The code for a step, from `HMAC-SHA1(secret, step)` by RFC 4226 dynamic truncation.
pub fn hotp_code(mac: &[u8; 20]) -> String {
    let offset = usize::from(mac[19] & 0x0f);
    let binary = u32::from_be_bytes([
        mac[offset] & 0x7f,
        mac[offset + 1],
        mac[offset + 2],
        mac[offset + 3],
    ]);
    let code = binary % 10u32.pow(TOTP_DIGITS);
    format!("{code:0width$}", width = TOTP_DIGITS as usize)
}

/// An authenticator app, set up once the user entered a first code from it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TotpCredential {
    pub user_id: UserId,
    pub sealed_secret: Vec<u8>,
    pub confirmed_at: Option<OffsetDateTime>,
    /// The last step a code was accepted for; older and equal steps are refused, so a code works
    /// once.
    pub last_used_step: Option<i64>,
}

impl TotpCredential {
    pub fn is_confirmed(&self) -> bool {
        self.confirmed_at.is_some()
    }
}

/// A user who passed the first step and still owes the second, stored under the digest of a
/// token in a short-lived cookie.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MfaChallenge {
    pub token_hash: TokenHash,
    pub user_id: UserId,
    pub attempts: u32,
    pub expires_at: OffsetDateTime,
}

impl MfaChallenge {
    pub fn start(user_id: UserId, token_hash: TokenHash, now: OffsetDateTime) -> Self {
        Self {
            token_hash,
            user_id,
            attempts: 0,
            expires_at: now.saturating_add(MFA_CHALLENGE_TTL),
        }
    }

    pub fn is_live(&self, now: OffsetDateTime) -> bool {
        now < self.expires_at && self.attempts < MAX_MFA_ATTEMPTS
    }
}
