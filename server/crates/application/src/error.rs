//! The application error type, returned by every use case.
//!
//! Each variant has a stable machine-readable [`AppError::code`]. The API layer maps variants to
//! HTTP statuses and RFC 9457 problem documents (`api::problem`); nothing here knows about HTTP.

use std::{
    error::Error as StdError,
    fmt::{self, Display, Formatter},
};

pub use domain::error::ErrorChain;
use domain::{
    error::{StorageError, ValidationError},
    i18n::Message,
    security::{CryptoError, HashError, TokenError},
};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldError {
    pub field: String,
    pub code: String,
    pub message: Message,
}

/// Every invalid field of a request, collected so the client can show them all at once.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ValidationErrors(Vec<FieldError>);

impl ValidationErrors {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn single(field: &str, error: &ValidationError) -> Self {
        let mut errors = Self::new();
        errors.add(field, error);
        errors
    }

    pub fn add(&mut self, field: &str, error: &ValidationError) {
        self.0.push(FieldError {
            field: field.to_owned(),
            code: error.code().to_owned(),
            message: error.message().clone(),
        });
    }

    /// Records the error of `result` under `field` and returns its value, if any, so every
    /// field is validated before giving up:
    ///
    /// ```
    /// # use application::ValidationErrors;
    /// # use domain::user::{Email, Username};
    /// # struct Registration { email: Email, username: Username }
    /// # fn validate(email: &str, username: &str) -> Result<Registration, ValidationErrors> {
    /// let mut errors = ValidationErrors::new();
    /// let email = errors.check("email", Email::parse(email));
    /// let username = errors.check("username", Username::parse(username));
    /// match (email, username) {
    ///     (Some(email), Some(username)) => Ok(Registration { email, username }),
    ///     _ => Err(errors),
    /// }
    /// # }
    /// ```
    pub fn check<T>(&mut self, field: &str, result: Result<T, ValidationError>) -> Option<T> {
        result.map_err(|error| self.add(field, &error)).ok()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn fields(&self) -> &[FieldError] {
        &self.0
    }

    pub fn into_fields(self) -> Vec<FieldError> {
        self.0
    }
}

impl Display for ValidationErrors {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let fields: Vec<&str> = self.0.iter().map(|error| error.field.as_str()).collect();
        write!(f, "invalid fields: {}", fields.join(", "))
    }
}

impl StdError for ValidationErrors {}

/// A failure the client cannot do anything about. Logged with its cause, shown to the client as a
/// generic 500.
#[derive(Debug, Error)]
pub enum InternalError {
    #[error(transparent)]
    Storage(StorageError),
    #[error(transparent)]
    Hash(#[from] HashError),
    #[error(transparent)]
    Token(#[from] TokenError),
    #[error(transparent)]
    Crypto(#[from] CryptoError),
}

/// Why a use case failed, in terms a client can act on. Only [`AppError::Internal`] is the
/// server's fault; every other variant is an expected outcome. The API layer renders them, keyed
/// by [`AppError::code`].
#[derive(Debug, Error)]
pub enum AppError {
    #[error("the request is invalid: {0}")]
    Validation(ValidationErrors),
    #[error("authentication required")]
    Unauthenticated,
    #[error("invalid email, username or password")]
    InvalidCredentials,
    #[error("verify your email address to sign in")]
    EmailNotVerified,
    #[error("this account has been disabled")]
    AccountDisabled,
    #[error("you do not have permission to do this")]
    Forbidden,
    /// A sensitive change and the session did not prove who is behind it recently; the client
    /// re-authenticates (`POST /me/reauthenticate`) and retries.
    #[error("confirm it's you to continue")]
    ReauthRequired,
    /// No such resource, or one the actor may not see. The two are deliberately the same, so ids
    /// cannot be probed.
    #[error("not found")]
    NotFound,
    #[error("conflict: {code}")]
    Conflict {
        code: &'static str,
        message: Message,
    },
    #[error("this link is invalid or has expired")]
    InvalidToken,
    #[error("this passkey could not be verified")]
    InvalidPasskey,
    #[error("the sign-in provider could not be reached, try again")]
    ProviderUnavailable,
    /// The server is at capacity for this kind of work (hashing passwords); the client retries
    /// after a moment.
    #[error("the server is busy, try again in a moment")]
    Busy,
    /// Unexpected; see [`InternalError`]. Clients only ever see a generic message.
    #[error("internal error")]
    Internal(#[from] InternalError),
}

impl AppError {
    pub fn conflict(code: &'static str, message: Message) -> Self {
        Self::Conflict { code, message }
    }

    pub fn invalid(field: &str, error: &ValidationError) -> Self {
        Self::Validation(ValidationErrors::single(field, error))
    }

    /// A one-time code (emailed, texted, from an authenticator app, or a recovery code) that is
    /// wrong, expired or already used. Every code is reported the same way, on the `code` field, so
    /// the form shows it next to the input.
    pub fn invalid_code() -> Self {
        Self::invalid(
            "code",
            &ValidationError::new("invalid_code", Message::new("validation-invalid-code")),
        )
    }

    /// Stable and machine-readable. Clients switch on it; never change an existing one.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Validation(_) => "validation_failed",
            Self::Unauthenticated => "unauthenticated",
            Self::InvalidCredentials => "invalid_credentials",
            Self::EmailNotVerified => "email_not_verified",
            Self::AccountDisabled => "account_disabled",
            Self::Forbidden => "forbidden",
            Self::ReauthRequired => "reauth_required",
            Self::NotFound => "not_found",
            Self::Conflict { code, .. } => code,
            Self::InvalidToken => "invalid_token",
            Self::InvalidPasskey => "invalid_passkey",
            Self::ProviderUnavailable => "provider_unavailable",
            Self::Busy => "busy",
            Self::Internal(_) => "internal_error",
        }
    }

    /// What to tell the person, as a catalog message (the `detail` of the problem document). The
    /// `#[error]` texts above are for logs and never reach a client; internal errors get a generic
    /// message and their cause stays in the log.
    pub fn message(&self) -> Message {
        match self {
            Self::Validation(_) => Message::new("error-validation-failed"),
            Self::Unauthenticated => Message::new("error-unauthenticated"),
            Self::InvalidCredentials => Message::new("error-invalid-credentials"),
            Self::EmailNotVerified => Message::new("error-email-not-verified"),
            Self::AccountDisabled => Message::new("error-account-disabled"),
            Self::Forbidden => Message::new("error-forbidden"),
            Self::ReauthRequired => Message::new("error-reauth-required"),
            Self::NotFound => Message::new("error-not-found"),
            Self::Conflict { message, .. } => message.clone(),
            Self::InvalidToken => Message::new("error-invalid-token"),
            Self::InvalidPasskey => Message::new("error-invalid-passkey"),
            Self::ProviderUnavailable => Message::new("error-provider-unavailable"),
            Self::Busy => Message::new("error-busy"),
            Self::Internal(_) => Message::new("error-internal"),
        }
    }
}

impl From<ValidationErrors> for AppError {
    fn from(errors: ValidationErrors) -> Self {
        Self::Validation(errors)
    }
}

// Storage failures are internal. Use cases that expect a specific constraint violation check for
// it before converting, and missing rows arrive as `Option`s.
impl From<StorageError> for AppError {
    fn from(err: StorageError) -> Self {
        Self::Internal(InternalError::Storage(err))
    }
}

impl From<HashError> for AppError {
    fn from(err: HashError) -> Self {
        match err {
            HashError::Busy => Self::Busy,
            HashError::Failed(_) => Self::Internal(err.into()),
        }
    }
}

impl From<TokenError> for AppError {
    fn from(err: TokenError) -> Self {
        Self::Internal(err.into())
    }
}

impl From<CryptoError> for AppError {
    fn from(err: CryptoError) -> Self {
        Self::Internal(err.into())
    }
}

pub(crate) fn username_taken() -> AppError {
    AppError::invalid(
        "username",
        &ValidationError::new("username_taken", Message::new("validation-username-taken")),
    )
}
