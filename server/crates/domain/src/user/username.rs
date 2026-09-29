use std::fmt::{self, Display, Formatter};

use crate::{error::ValidationError, i18n::Message};

pub const MIN_USERNAME_LEN: usize = 3;
pub const MAX_USERNAME_LEN: usize = 30;

/// A lowercased handle to sign in with instead of the email address.
///
/// 3 to [`MAX_USERNAME_LEN`] characters of `a-z`, `0-9`, `.`, `_` and `-`, starting with a letter
/// or digit and containing at least one letter. The rules keep usernames apart from the other
/// sign-in identifiers: without an `@` one is never an email address, and with a letter it is
/// never a phone number.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Username(String);

impl Username {
    pub fn parse(raw: &str) -> Result<Self, ValidationError> {
        let name = raw.trim().to_lowercase();
        let len = name.chars().count();

        if len == 0 {
            Err(ValidationError::required())
        } else if len < MIN_USERNAME_LEN {
            Err(ValidationError::new(
                "too_short",
                Message::new("validation-username-too-short").arg("min", MIN_USERNAME_LEN),
            ))
        } else if len > MAX_USERNAME_LEN {
            Err(ValidationError::new(
                "too_long",
                Message::new("validation-username-too-long").arg("max", MAX_USERNAME_LEN),
            ))
        } else if !name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'))
            || !name.starts_with(|c: char| c.is_ascii_alphanumeric())
        {
            Err(ValidationError::new(
                "invalid_username",
                Message::new("validation-username-invalid-characters"),
            ))
        } else if !name.chars().any(|c| c.is_ascii_lowercase()) {
            Err(ValidationError::new(
                "invalid_username",
                Message::new("validation-username-needs-letter"),
            ))
        } else {
            Ok(Self(name))
        }
    }

    /// A username made from free text such as a name or the local part of an email address, for
    /// accounts whose owner did not pick one. Characters a username cannot hold are dropped, and
    /// `suffix` (such as random digits that make it unique) is appended. `None` if too little of
    /// `hint` survives.
    pub fn suggest(hint: &str, suffix: &str) -> Option<Self> {
        let base: String = hint
            .to_lowercase()
            .chars()
            .filter(|c| {
                c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-')
            })
            .skip_while(|c| !c.is_ascii_alphanumeric())
            .take(MAX_USERNAME_LEN.saturating_sub(suffix.chars().count()))
            .collect();
        Self::parse(&format!("{base}{suffix}")).ok()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for Username {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
