use std::{
    fmt::{self, Debug, Formatter},
    sync::LazyLock,
};

use crate::{error::ValidationError, i18n::Message, secret::Secret};

pub const MIN_PASSWORD_LEN: usize = 8;
pub const MAX_PASSWORD_LEN: usize = 128;

/// The passwords of 8 to 128 characters among the 100 000 most used in breaches (the UK NCSC's
/// list, via `SecLists`, MIT licensed), lowercased and sorted bytewise. NIST SP 800-63B asks new
/// passwords to be checked against such a list. The check runs offline, so no password leaves the
/// server. To use another list, replace the file and keep it sorted.
const COMMON_PASSWORDS: &str = include_str!("common_passwords.txt");

static COMMON: LazyLock<Vec<&'static str>> = LazyLock::new(|| COMMON_PASSWORDS.lines().collect());

pub fn is_common_password(password: &str) -> bool {
    COMMON
        .binary_search(&password.to_ascii_lowercase().as_str())
        .is_ok()
}

/// A password that satisfies the policy for new passwords.
///
/// Existing passwords are only ever compared against their hash, so they stay a plain [`Secret`]:
/// tightening the policy must not lock anyone out.
pub struct NewPassword(Secret);

impl NewPassword {
    /// Checks the policy: [`MIN_PASSWORD_LEN`] to [`MAX_PASSWORD_LEN`] characters (counted as
    /// characters, not bytes), not only spaces, not [`is_common_password`].
    ///
    /// # Errors
    ///
    /// Fails with `required`, `too_short`, `too_long`, `too_weak` or `too_common`.
    pub fn parse(password: Secret) -> Result<Self, ValidationError> {
        let len = password.expose().chars().count();

        if len == 0 {
            Err(ValidationError::required())
        } else if len < MIN_PASSWORD_LEN {
            Err(ValidationError::new(
                "too_short",
                Message::new("validation-password-too-short").arg("min", MIN_PASSWORD_LEN),
            ))
        } else if len > MAX_PASSWORD_LEN {
            Err(ValidationError::new(
                "too_long",
                Message::new("validation-password-too-long").arg("max", MAX_PASSWORD_LEN),
            ))
        } else if password.expose().trim().is_empty() {
            Err(ValidationError::new(
                "too_weak",
                Message::new("validation-password-only-spaces"),
            ))
        } else if is_common_password(password.expose()) {
            Err(ValidationError::new(
                "too_common",
                Message::new("validation-password-too-common"),
            ))
        } else {
            Ok(Self(password))
        }
    }

    pub fn as_secret(&self) -> &Secret {
        &self.0
    }
}

impl Debug for NewPassword {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("NewPassword(<redacted>)")
    }
}

/// A password hash in PHC string format, such as `$argon2id$v=19$m=19456,t=2,p=1$...`.
///
/// Redacted from `Debug`: a hash is not the password, but it is still an offline brute-force
/// target.
#[derive(Clone, PartialEq, Eq)]
pub struct PasswordHash(String);

impl PasswordHash {
    pub fn new(phc: impl Into<String>) -> Self {
        Self(phc.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Debug for PasswordHash {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str("PasswordHash(<redacted>)")
    }
}
