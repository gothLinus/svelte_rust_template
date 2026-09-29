use crate::{
    error::ValidationError,
    user::{Email, PhoneNumber, Username},
};

/// An email address, a username or a phone number.
///
/// Told apart by shape: an `@` makes an email address; a leading `+` or nothing but digits and
/// separators makes a phone number; anything else is a username (usernames always contain a
/// letter and never an `@`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginIdentifier {
    Email(Email),
    Username(Username),
    Phone(PhoneNumber),
}

impl LoginIdentifier {
    pub fn parse(raw: &str) -> Result<Self, ValidationError> {
        let raw = raw.trim();
        if raw.is_empty() {
            return Err(ValidationError::required());
        }
        if raw.contains('@') {
            return Email::parse(raw).map(Self::Email);
        }
        let phone_like = raw.starts_with('+')
            || raw
                .chars()
                .all(|c| c.is_ascii_digit() || matches!(c, ' ' | '-' | '.' | '(' | ')'));
        if phone_like {
            PhoneNumber::parse(raw).map(Self::Phone)
        } else {
            Username::parse(raw).map(Self::Username)
        }
    }
}
