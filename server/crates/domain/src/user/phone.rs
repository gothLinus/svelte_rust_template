use std::fmt::{self, Display, Formatter};

use crate::{error::ValidationError, i18n::Message};

/// A phone number in E.164 form: `+`, a country code and up to 15 digits in total.
///
/// Spaces, dashes, dots and parentheses are dropped while parsing, so `+49 (170) 123-4567`
/// becomes `+491701234567`. Only a text message proves the number exists and belongs to the user.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PhoneNumber(String);

impl PhoneNumber {
    pub fn parse(raw: &str) -> Result<Self, ValidationError> {
        let compact: String = raw
            .trim()
            .chars()
            .filter(|c| !matches!(c, ' ' | '-' | '.' | '(' | ')'))
            .collect();
        if compact.is_empty() {
            return Err(ValidationError::required());
        }

        let valid = compact.strip_prefix('+').is_some_and(|digits| {
            (7..=15).contains(&digits.len())
                && digits.chars().all(|c| c.is_ascii_digit())
                && !digits.starts_with('0')
        });
        if valid {
            Ok(Self(compact))
        } else {
            Err(ValidationError::new(
                "invalid_phone",
                Message::new("validation-invalid-phone"),
            ))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn masked(&self) -> String {
        let digits = &self.0[1..];
        let country = &digits[..digits.len().min(2)];
        let tail = &digits[digits.len().saturating_sub(2)..];
        let hidden = digits.len().saturating_sub(country.len() + tail.len());
        format!("+{country}{}{tail}", "•".repeat(hidden))
    }
}

impl fmt::Debug for PhoneNumber {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "PhoneNumber({})", self.masked())
    }
}

impl Display for PhoneNumber {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A country calling code such as `+41`: `+`, then one to three digits, not starting with `0`.
/// Used to allow texts only to the countries a product serves.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CallingCode(String);

impl CallingCode {
    pub fn parse(raw: &str) -> Result<Self, ValidationError> {
        let raw = raw.trim();
        let valid = raw.strip_prefix('+').is_some_and(|digits| {
            (1..=3).contains(&digits.len())
                && digits.chars().all(|c| c.is_ascii_digit())
                && !digits.starts_with('0')
        });
        if valid {
            Ok(Self(raw.to_owned()))
        } else {
            Err(ValidationError::new(
                "invalid_calling_code",
                Message::new("validation-invalid-calling-code").arg("code", raw),
            ))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn covers(&self, phone: &PhoneNumber) -> bool {
        phone.as_str().starts_with(&self.0)
    }
}
