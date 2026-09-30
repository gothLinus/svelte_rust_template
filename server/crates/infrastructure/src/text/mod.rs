//! Text message transports: Twilio (SMS and WhatsApp), the log for development, and a disabled
//! transport for deployments without texting.
//!
//! Every transport implements `domain::text::TextSender`; [`start`] picks one from `TextConfig`.
//! To add a provider, implement `TextSender` (reporting the channels it can deliver on), add a
//! `TextTransport` variant in `config::texts` and a match arm in [`start`].

use std::sync::Arc;

use domain::{
    one_time_code::CodeChannel,
    text::{TextFuture, TextMessage, TextSender},
};

use crate::config::{TextConfig, TextTransport};

pub use log::LogTextSender;
pub use twilio::TwilioTextSender;

mod log;
mod twilio;

/// The sender the configuration asks for. Production delivers through it from the outbox
/// (`crate::outbox`); its `channels` decide which delivery channels the API offers.
pub fn start(config: &TextConfig, http: &reqwest::Client) -> Arc<dyn TextSender> {
    match &config.transport {
        TextTransport::Disabled => Arc::new(DisabledTextSender),
        TextTransport::Log => Arc::new(LogTextSender),
        TextTransport::Twilio(twilio) => {
            Arc::new(TwilioTextSender::new(twilio.clone(), http.clone()))
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DisabledTextSender;

impl TextSender for DisabledTextSender {
    fn channels(&self) -> &[CodeChannel] {
        &[]
    }

    fn send(&self, _message: TextMessage) -> TextFuture<'_> {
        Box::pin(async { Ok(()) })
    }
}
