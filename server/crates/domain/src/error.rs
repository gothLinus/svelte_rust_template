use std::{
    error::Error as StdError,
    fmt::{self, Display, Formatter},
};

use thiserror::Error;

use crate::i18n::Message;

/// A value that breaks a domain rule, such as an email address without an `@`.
///
/// `code` is stable and meant for machines. `message` selects the sentence shown to the person
/// who typed the value (see [`crate::i18n`]); one code can come with different messages. The
/// `Display` output is for logs only.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("{code} ({})", .message.id())]
pub struct ValidationError {
    code: &'static str,
    message: Message,
}

impl ValidationError {
    pub fn new(code: &'static str, message: Message) -> Self {
        Self { code, message }
    }

    pub fn required() -> Self {
        Self::new("required", Message::new("validation-required"))
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn message(&self) -> &Message {
        &self.message
    }
}

/// A stored value that no variant of the domain has, such as a purpose name written by another
/// version of the code.
///
/// Never a person's mistake, so unlike [`ValidationError`] it has no message; adapters report it
/// as [`StorageError::Corrupt`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("unknown {kind} `{value}`")]
pub struct UnknownValue {
    kind: &'static str,
    value: String,
}

impl UnknownValue {
    pub fn new(kind: &'static str, value: impl Into<String>) -> Self {
        Self {
            kind,
            value: value.into(),
        }
    }
}

/// A failed storage operation, classified so callers can react to the cases they expect.
///
/// Adapters translate their driver's errors into these variants, so no driver type reaches the
/// application layer.
#[derive(Debug, Error)]
pub enum StorageError {
    /// A unique constraint rejected the write; `constraint` is its name in the schema.
    #[error("unique constraint `{constraint}` violated")]
    UniqueViolation { constraint: String },
    #[error("foreign key constraint `{constraint}` violated")]
    ForeignKeyViolation { constraint: String },
    #[error("stored data is invalid")]
    Corrupt(#[source] Box<dyn StdError + Send + Sync>),
    #[error("storage backend failed")]
    Backend(#[source] Box<dyn StdError + Send + Sync>),
}

impl StorageError {
    pub fn backend(err: impl StdError + Send + Sync + 'static) -> Self {
        Self::Backend(Box::new(err))
    }

    pub fn corrupt(err: impl StdError + Send + Sync + 'static) -> Self {
        Self::Corrupt(Box::new(err))
    }

    /// Whether this is a [`StorageError::UniqueViolation`] of the constraint called `name`, so
    /// callers can turn an expected conflict into a friendly error and let the rest through.
    pub fn is_unique_violation(&self, name: &str) -> bool {
        matches!(self, Self::UniqueViolation { constraint } if constraint == name)
    }
}

/// Displays an error followed by every `source()` below it, separated by `: `.
///
/// Use it wherever an error is logged, so the root cause is not lost.
pub struct ErrorChain<'a>(pub &'a (dyn StdError + 'static));

impl Display for ErrorChain<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)?;
        let mut source = self.0.source();
        while let Some(err) = source {
            write!(f, ": {err}")?;
            source = err.source();
        }
        Ok(())
    }
}
