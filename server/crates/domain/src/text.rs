use std::{error::Error as StdError, future::Future, pin::Pin};

use thiserror::Error;
use time::Duration;

use crate::{one_time_code::CodeChannel, user::PhoneNumber};

#[derive(Debug, Error)]
#[error("sending a text message failed")]
pub struct TextError(#[source] Box<dyn StdError + Send + Sync>);

impl TextError {
    pub fn new(err: impl StdError + Send + Sync + 'static) -> Self {
        Self(Box::new(err))
    }
}

/// A short message to one phone number. The body holds a sign-in code, so it must not be logged
/// outside of development.
#[derive(Clone)]
pub struct TextMessage {
    pub to: PhoneNumber,
    pub channel: CodeChannel,
    pub body: String,
    pub valid_for: Duration,
}

impl std::fmt::Debug for TextMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextMessage")
            .field("to", &self.to)
            .field("channel", &self.channel)
            .field("body", &"<redacted>")
            .field("valid_for", &self.valid_for)
            .finish()
    }
}

pub type TextFuture<'a> = Pin<Box<dyn Future<Output = Result<(), TextError>> + Send + 'a>>;

/// Delivers text messages. Used through `dyn` for the same reason as
/// [`Mailer`](crate::mail::Mailer): the transport is picked from configuration.
pub trait TextSender: Send + Sync + 'static {
    fn channels(&self) -> &[CodeChannel];

    fn send(&self, message: TextMessage) -> TextFuture<'_>;
}
