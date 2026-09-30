use std::{error::Error as StdError, future::Future, pin::Pin};

use thiserror::Error;
use time::Duration;

use crate::user::Email;

#[derive(Debug, Error)]
#[error("sending mail failed")]
pub struct MailError(#[source] Box<dyn StdError + Send + Sync>);

impl MailError {
    pub fn new(err: impl StdError + Send + Sync + 'static) -> Self {
        Self(Box::new(err))
    }
}

/// A plain-text message to a single recipient.
///
/// The body may contain single-use tokens and codes, so it must not be logged outside of
/// development. Logs name the [`template`](Mail::template) instead.
#[derive(Clone)]
pub struct Mail {
    pub to: Email,
    pub template: String,
    pub subject: String,
    pub body: String,
    /// How long delivering it is worth: once the link or code in it expired, a late delivery only
    /// confuses. `None` for notices, which transports retry as long as they retry anything.
    pub valid_for: Option<Duration>,
}

impl std::fmt::Debug for Mail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Mail")
            .field("to", &self.to)
            .field("template", &self.template)
            .field("subject", &self.subject)
            .field("body", &"<redacted>")
            .field("valid_for", &self.valid_for)
            .finish()
    }
}

pub type MailFuture<'a> = Pin<Box<dyn Future<Output = Result<(), MailError>> + Send + 'a>>;

/// Delivers mail.
///
/// One of the three ports used through `dyn`, with [`TextSender`](crate::text::TextSender) and
/// [`IdentityProviders`](crate::identity::IdentityProviders). The transport is picked at runtime, and a type parameter
/// would add one to every service for no gain: mail is off the hot path. `send` returns a
/// [`MailFuture`] because `async fn` in traits is not object safe.
pub trait Mailer: Send + Sync + 'static {
    /// Hands `mail` to the transport. `Ok` means the transport accepted it, not that the recipient
    /// has it: behind the outbox it means "queued for delivery", and delivery failures are retried
    /// there.
    fn send(&self, mail: Mail) -> MailFuture<'_>;
}
