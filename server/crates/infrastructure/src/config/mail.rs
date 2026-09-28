use domain::secret::Secret;
use lettre::message::Mailbox;

use super::reader::Reader;

#[derive(Debug, Clone)]
pub enum MailTransport {
    Smtp {
        url: Secret,
    },
    /// Logs mail instead of sending it (development only: the log contains the links).
    Log,
}

#[derive(Debug, Clone)]
pub struct MailConfig {
    pub transport: MailTransport,
    pub from: Mailbox,
}

impl Reader<'_> {
    /// Off localhost the transport must be chosen explicitly, and `log` is refused: a forgotten
    /// variable would otherwise start a server that never delivers a verification, reset or
    /// security mail and writes working links to its log.
    pub(super) fn mail(&mut self) -> Option<MailConfig> {
        let from = self.required("MAIL_FROM", |raw| {
            raw.parse::<Mailbox>()
                .map_err(|_| "must be a mailbox, e.g. `Example <noreply@example.com>`".to_owned())
        });
        let transport = match self.raw("MAIL_TRANSPORT").as_deref() {
            None if !self.local => {
                self.problem(
                    "MAIL_TRANSPORT",
                    "is required when APP_URL is not localhost: set it to `smtp`",
                );
                None
            }
            Some("log") if !self.local => {
                self.problem(
                    "MAIL_TRANSPORT",
                    "`log` only works on localhost: mail would be logged instead of delivered",
                );
                None
            }
            None | Some("log") => Some(MailTransport::Log),
            Some("smtp") => {
                let local = self.local;
                self.required("SMTP_URL", |raw| {
                    if !local && sends_credentials_in_the_clear(raw) {
                        return Err("sends its credentials in the clear: use smtps://, or add \
                                    ?tls=required to smtp://"
                            .to_owned());
                    }
                    Ok(Secret::new(raw))
                })
                .map(|url| MailTransport::Smtp { url })
            }
            Some(_) => {
                self.problem("MAIL_TRANSPORT", "must be `smtp` or `log`");
                None
            }
        };

        Some(MailConfig {
            transport: transport?,
            from: from?,
        })
    }
}

/// Whether an SMTP URL carries a user name or password but would not insist on TLS:
/// `smtp://user:pass@host` without `tls=required` falls back to plain text when the server does
/// not offer STARTTLS, or an attacker strips it.
fn sends_credentials_in_the_clear(url: &str) -> bool {
    let Some((scheme, rest)) = url.split_once("://") else {
        return false;
    };
    let authority = rest.split(['/', '?']).next().unwrap_or_default();
    let has_credentials = authority.contains('@');
    let query = rest.split_once('?').map_or("", |(_, query)| query);
    let tls_required = query
        .split('&')
        .any(|pair| pair.eq_ignore_ascii_case("tls=required"));
    has_credentials && scheme.eq_ignore_ascii_case("smtp") && !tls_required
}
