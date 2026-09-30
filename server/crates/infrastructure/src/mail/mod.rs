//! Mail transports: SMTP for real delivery and the log for development. Production queues mail in
//! the database (`crate::outbox`); [`MailQueue`] is an in-memory alternative.
//!
//! Every transport implements `domain::mail::Mailer`, the port the services send through;
//! [`transport`] picks one from `MailConfig`. To add a transport (an API-based sender, say),
//! implement `Mailer`, add a `MailTransport` variant in `config::mail` and a match arm in
//! [`transport`].

use std::sync::Arc;

use domain::mail::Mailer;

use crate::config::{MailConfig, MailTransport};

pub use log::LogMailer;
pub use queue::{MailQueue, MailWorker, QUEUE_CAPACITY, QueueFull};
pub use smtp::{InvalidSmtpUrl, SmtpMailer};

mod log;
mod queue;
mod smtp;

/// The transport the configuration asks for. Production delivers through it from the outbox
/// (`crate::outbox`).
pub fn transport(config: &MailConfig) -> Result<Arc<dyn Mailer>, InvalidSmtpUrl> {
    Ok(match &config.transport {
        MailTransport::Smtp { url } => Arc::new(SmtpMailer::new(url, config.from.clone())?),
        MailTransport::Log => Arc::new(LogMailer),
    })
}

/// The transport behind an in-memory [`MailQueue`], for running without a database (the queue
/// loses what it holds on a crash). Must be called inside a Tokio runtime.
pub fn start(config: &MailConfig) -> Result<(Arc<dyn Mailer>, MailWorker), InvalidSmtpUrl> {
    let (queue, worker) = MailQueue::start(transport(config)?);
    Ok((Arc::new(queue), worker))
}
