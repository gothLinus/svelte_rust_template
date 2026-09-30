use domain::mail::{Mail, MailFuture, Mailer};

/// Writes mail to the log instead of sending it. For development without an SMTP server.
///
/// The masked recipient and the template are logged at `info`; the body, which contains
/// verification and reset links and sign-in codes, only at `debug` under the `mail` target. Never
/// use this transport in production: the log would hold working account-recovery links.
#[derive(Debug, Clone, Copy, Default)]
pub struct LogMailer;

impl Mailer for LogMailer {
    fn send(&self, mail: Mail) -> MailFuture<'_> {
        tracing::info!(
            target: "mail",
            to = %mail.to.masked(),
            template = %mail.template,
            "mail not sent (MAIL_TRANSPORT=log)"
        );
        tracing::debug!(target: "mail", to = %mail.to, body = %mail.body, "mail body");
        Box::pin(async { Ok(()) })
    }
}
