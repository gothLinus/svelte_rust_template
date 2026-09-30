use domain::{
    mail::{Mail, MailError, MailFuture, Mailer},
    secret::Secret,
};
use lettre::{
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
    message::{Mailbox, header::ContentType},
    transport::smtp,
};
use thiserror::Error;

#[derive(Debug, Error)]
#[error("SMTP_URL is not a valid smtp:// or smtps:// URL")]
pub struct InvalidSmtpUrl(#[source] smtp::Error);

pub struct SmtpMailer {
    transport: AsyncSmtpTransport<Tokio1Executor>,
    from: Mailbox,
}

impl SmtpMailer {
    /// Connecting is lazy, so an unreachable server only shows up when the first mail is sent, and
    /// is logged then.
    pub fn new(url: &Secret, from: Mailbox) -> Result<Self, InvalidSmtpUrl> {
        let transport = AsyncSmtpTransport::<Tokio1Executor>::from_url(url.expose())
            .map_err(InvalidSmtpUrl)?
            .build();
        Ok(Self { transport, from })
    }
}

impl Mailer for SmtpMailer {
    fn send(&self, mail: Mail) -> MailFuture<'_> {
        Box::pin(async move {
            let to = mail.to.as_str().parse().map_err(MailError::new)?;
            let message = Message::builder()
                .from(self.from.clone())
                .to(Mailbox::new(None, to))
                .subject(mail.subject)
                .header(ContentType::TEXT_PLAIN)
                .body(mail.body)
                .map_err(MailError::new)?;

            self.transport
                .send(message)
                .await
                .map(drop)
                .map_err(MailError::new)
        })
    }
}
