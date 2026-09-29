use std::fmt::{self, Display, Formatter};

use crate::{error::ValidationError, i18n::Message};

/// RFC 5321 caps a forward path at 256 octets including the angle brackets.
pub const MAX_EMAIL_LEN: usize = 254;

/// A trimmed, lowercased email address.
///
/// Lowercasing makes addresses compare case-insensitively everywhere (lookups, the unique index,
/// rate limit keys). Strictly the local part is case-sensitive, but no mainstream provider treats
/// it that way, and two accounts that differ only in case are always a mistake.
///
/// Validation is deliberately shallow: one `@`, a non-empty local part and a dotted domain. Only
/// sending mail proves an address exists.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct Email(String);

impl Email {
    pub fn parse(raw: &str) -> Result<Self, ValidationError> {
        let email = raw.trim().to_lowercase();
        if email.is_empty() {
            return Err(ValidationError::required());
        }

        let valid = email.split_once('@').is_some_and(|(local, domain)| {
            email.len() <= MAX_EMAIL_LEN
                && !local.is_empty()
                && !domain.contains('@')
                && domain.contains('.')
                && !domain.starts_with('.')
                && !domain.ends_with('.')
                && !domain.contains("..")
                && !email.chars().any(|c| c.is_whitespace() || c.is_control())
        });

        if valid {
            Ok(Self(email))
        } else {
            Err(ValidationError::new(
                "invalid_email",
                Message::new("validation-invalid-email"),
            ))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The address with most of the local part hidden (`a***@example.com`), for logs: full
    /// addresses are personal data and stay out of info-level logs.
    pub fn masked(&self) -> String {
        match self.0.split_once('@') {
            Some((local, domain)) => {
                let first = local.chars().next().unwrap_or('*');
                format!("{first}***@{domain}")
            }
            None => "***".to_owned(),
        }
    }
}

impl fmt::Debug for Email {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "Email({})", self.masked())
    }
}

impl Display for Email {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
