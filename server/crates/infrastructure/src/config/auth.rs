//! `APP_NAME`, `SECRET_KEY`, sessions, emailed links, and Argon2.
//!
//! `SECRET_KEY` is required and refused if it looks non-random, if it is the published development
//! key off localhost, or if `SECRET_KEY_PREVIOUS` equals it. Durations that feed the session and
//! token policies are bounded, and the idle timeout may not exceed the session lifetime. Argon2
//! costs are validated by `Argon2Params::validate`.

use std::fmt::{self, Formatter};

use base64::{
    Engine, alphabet,
    engine::{DecodePaddingMode, GeneralPurpose, GeneralPurposeConfig},
};
use domain::{session::SessionPolicy, user_token::TokenPolicy};
use zeroize::Zeroizing;

use super::reader::{DAY, Reader, signed, unsigned};
use crate::crypto::{Argon2Param, Argon2Params, SECRET_KEY_BYTES};

/// The `SECRET_KEY` `.env.example` used to publish, accepted only on localhost.
const DEVELOPMENT_KEY: &[u8; SECRET_KEY_BYTES] = b"development-only-key-change-me!!";

#[derive(Debug, Clone)]
pub struct AuthConfig {
    pub app_name: String,
    /// Encrypts secrets the server must read back (authenticator app seeds) and keys the digests of
    /// short codes.
    pub secret_key: SecretKey,
    /// `SECRET_KEY_PREVIOUS`: the key before a rotation. What it sealed still opens and its
    /// recovery-code digests still match; new values use `secret_key`. Remove it once every user
    /// has re-enrolled their authenticator app and recovery codes.
    pub previous_secret_key: Option<SecretKey>,
    pub sessions: SessionPolicy,
    pub tokens: TokenPolicy,
    pub require_email_verification: bool,
    /// `UNVERIFIED_ACCOUNT_TTL`: accounts still unverified this long after registering are deleted.
    /// Defaults to 7 days with `REQUIRE_EMAIL_VERIFICATION=true`, where such an account cannot even
    /// sign in, and to keeping them otherwise, since there they are in use; `0` keeps them.
    pub unverified_account_ttl: Option<time::Duration>,
    pub argon2: Argon2Params,
}

#[derive(Clone)]
pub struct SecretKey(Zeroizing<[u8; SECRET_KEY_BYTES]>);

impl SecretKey {
    pub fn new(bytes: [u8; SECRET_KEY_BYTES]) -> Self {
        Self(Zeroizing::new(bytes))
    }

    pub fn parse(raw: &str) -> Result<Self, String> {
        const LENIENT: GeneralPurposeConfig =
            GeneralPurposeConfig::new().with_decode_padding_mode(DecodePaddingMode::Indifferent);
        let raw = raw.trim();
        let bytes = GeneralPurpose::new(&alphabet::STANDARD, LENIENT)
            .decode(raw)
            .or_else(|_| GeneralPurpose::new(&alphabet::URL_SAFE, LENIENT).decode(raw))
            .map_err(|_| KEY_ERROR.to_owned())?;
        let bytes: [u8; SECRET_KEY_BYTES] = bytes.try_into().map_err(|_| KEY_ERROR.to_owned())?;
        Ok(Self::new(bytes))
    }

    pub fn bytes(&self) -> &[u8; SECRET_KEY_BYTES] {
        &self.0
    }

    /// Whether the key has too few distinct bytes to be random: 32 bytes from a generator have
    /// about 30, and fewer than 16 happens by chance with a probability far below 2^-100. Catches
    /// placeholders like 32 zero bytes or a repeated word.
    pub fn looks_weak(&self) -> bool {
        let mut seen = [false; 256];
        for &byte in self.0.iter() {
            seen[usize::from(byte)] = true;
        }
        seen.iter().filter(|&&seen| seen).count() < MIN_DISTINCT_KEY_BYTES
    }

    pub fn is_development_key(&self) -> bool {
        &*self.0 == DEVELOPMENT_KEY
    }
}

const MIN_DISTINCT_KEY_BYTES: usize = 16;

const KEY_ERROR: &str = "must be 32 random bytes, base64-encoded: `openssl rand -base64 32`";

impl fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("SecretKey(<redacted>)")
    }
}

impl Reader<'_> {
    pub(super) fn auth(&mut self) -> Option<AuthConfig> {
        let year = 365 * DAY;
        let week = 7 * DAY;

        let defaults = SessionPolicy::default();
        let idle_timeout = self.duration(
            "SESSION_IDLE_TIMEOUT",
            unsigned(defaults.idle_timeout),
            year,
        );
        let lifetime = self.duration(
            "SESSION_LIFETIME",
            unsigned(defaults.absolute_lifetime),
            year,
        );
        if idle_timeout.is_zero() || idle_timeout > lifetime {
            self.problem(
                "SESSION_IDLE_TIMEOUT",
                "must be greater than zero and at most SESSION_LIFETIME",
            );
        }

        let token_defaults = TokenPolicy::default();
        let verification_ttl = self.duration(
            "EMAIL_VERIFICATION_TTL",
            unsigned(token_defaults.email_verification_ttl),
            week,
        );
        let reset_ttl = self.duration(
            "PASSWORD_RESET_TTL",
            unsigned(token_defaults.password_reset_ttl),
            week,
        );
        let magic_link_ttl = self.duration(
            "MAGIC_LINK_TTL",
            unsigned(token_defaults.magic_link_ttl),
            week,
        );
        for (name, ttl) in [
            ("EMAIL_VERIFICATION_TTL", verification_ttl),
            ("PASSWORD_RESET_TTL", reset_ttl),
            ("MAGIC_LINK_TTL", magic_link_ttl),
        ] {
            if ttl.is_zero() {
                self.problem(name, "must be greater than zero");
            }
        }
        let app_name = self.raw("APP_NAME").unwrap_or_else(|| "Acme".to_owned());
        let (secret_key, previous_secret_key) = self.secret_keys();
        let require_email_verification = self.bool("REQUIRE_EMAIL_VERIFICATION", false);
        let unverified_default = if require_email_verification {
            7 * DAY
        } else {
            std::time::Duration::ZERO
        };
        let unverified_account_ttl =
            Some(self.duration("UNVERIFIED_ACCOUNT_TTL", unverified_default, year))
                .filter(|ttl| !ttl.is_zero())
                .map(signed);

        let argon2_defaults = Argon2Params::default();
        let argon2 = Argon2Params {
            memory_kib: self.number("ARGON2_MEMORY_KIB", argon2_defaults.memory_kib),
            iterations: self.number("ARGON2_ITERATIONS", argon2_defaults.iterations),
            parallelism: self.number("ARGON2_PARALLELISM", argon2_defaults.parallelism),
            max_concurrent: self.number("ARGON2_MAX_CONCURRENT", argon2_defaults.max_concurrent),
            max_queued: self.number("ARGON2_MAX_QUEUED", argon2_defaults.max_queued),
        };
        if let Err((param, message)) = argon2.validate() {
            let variable = match param {
                Argon2Param::MemoryKib => "ARGON2_MEMORY_KIB",
                Argon2Param::Iterations => "ARGON2_ITERATIONS",
                Argon2Param::Parallelism => "ARGON2_PARALLELISM",
                Argon2Param::MaxConcurrent => "ARGON2_MAX_CONCURRENT",
            };
            self.problem(variable, &message);
        }

        Some(AuthConfig {
            app_name,
            secret_key: secret_key?,
            previous_secret_key,
            sessions: SessionPolicy {
                idle_timeout: signed(idle_timeout),
                absolute_lifetime: signed(lifetime),
                touch_interval: defaults.touch_interval,
            },
            tokens: TokenPolicy {
                email_verification_ttl: signed(verification_ttl),
                password_reset_ttl: signed(reset_ttl),
                email_change_ttl: token_defaults.email_change_ttl,
                magic_link_ttl: signed(magic_link_ttl),
            },
            require_email_verification,
            unverified_account_ttl,
            argon2,
        })
    }
}

impl Reader<'_> {
    fn secret_keys(&mut self) -> (Option<SecretKey>, Option<SecretKey>) {
        let secret_key = self.required("SECRET_KEY", SecretKey::parse);
        let previous_secret_key = self.optional("SECRET_KEY_PREVIOUS", SecretKey::parse);
        if secret_key.as_ref().is_some_and(SecretKey::looks_weak) {
            self.problem(
                "SECRET_KEY",
                "does not look random (too few distinct bytes); generate one with \
                     `openssl rand -base64 32`",
            );
        }
        if let (Some(current), Some(previous)) = (&secret_key, &previous_secret_key)
            && current.bytes() == previous.bytes()
        {
            self.problem(
                "SECRET_KEY_PREVIOUS",
                "is the same as SECRET_KEY; set it to the key being rotated out",
            );
        }
        if !self.local
            && secret_key
                .as_ref()
                .is_some_and(SecretKey::is_development_key)
        {
            self.problem(
                "SECRET_KEY",
                "is the published development key; generate one with `openssl rand -base64 32`",
            );
        }
        (secret_key, previous_secret_key)
    }
}
