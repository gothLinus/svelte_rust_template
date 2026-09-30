use std::sync::Arc;

use domain::{
    one_time_code::CodeChannel,
    text::{TextError, TextFuture, TextMessage, TextSender},
};
use thiserror::Error;

use crate::config::TwilioConfig;

const API_BASE: &str = "https://api.twilio.com/2010-04-01";

#[derive(Debug, Error)]
#[error("Twilio answered {status}: {body}")]
struct TwilioRejected {
    status: reqwest::StatusCode,
    body: String,
}

#[derive(Debug, Error)]
#[error("Twilio does not deliver email")]
struct UnsupportedChannel;

/// Sends through `POST /Accounts/{sid}/Messages.json`. WhatsApp uses the same endpoint with
/// `whatsapp:`-prefixed numbers and needs an approved sender.
///
/// Sending waits for Twilio. The application sends through the outbox (`crate::outbox`), whose
/// worker calls this in the background, so a request never waits for Twilio and its timing does
/// not reveal whether a number has an account.
pub struct TwilioTextSender {
    inner: Arc<Inner>,
    channels: Vec<CodeChannel>,
}

struct Inner {
    config: TwilioConfig,
    http: reqwest::Client,
    api_base: String,
}

impl TwilioTextSender {
    pub fn new(config: TwilioConfig, http: reqwest::Client) -> Self {
        let mut channels = Vec::new();
        if config.sms_from.is_some() {
            channels.push(CodeChannel::Sms);
        }
        if config.whatsapp_from.is_some() {
            channels.push(CodeChannel::Whatsapp);
        }
        Self {
            inner: Arc::new(Inner {
                config,
                http,
                api_base: API_BASE.to_owned(),
            }),
            channels,
        }
    }

    #[must_use]
    pub fn with_api_base(self, api_base: &str) -> Self {
        let inner = Inner {
            config: self.inner.config.clone(),
            http: self.inner.http.clone(),
            api_base: api_base.trim_end_matches('/').to_owned(),
        };
        Self {
            inner: Arc::new(inner),
            channels: self.channels,
        }
    }
}

impl Inner {
    async fn deliver(&self, message: TextMessage) -> Result<(), TextError> {
        let (sender, prefix) = match message.channel {
            CodeChannel::Sms => (self.config.sms_from.as_ref(), ""),
            CodeChannel::Whatsapp => (self.config.whatsapp_from.as_ref(), "whatsapp:"),
            CodeChannel::Email => return Err(TextError::new(UnsupportedChannel)),
        };
        let Some(sender) = sender else {
            return Err(TextError::new(TwilioRejected {
                status: reqwest::StatusCode::BAD_REQUEST,
                body: format!("no sender configured for {}", message.channel),
            }));
        };
        let from = format!("{prefix}{sender}");
        let to = format!("{prefix}{}", message.to);

        let url = format!(
            "{}/Accounts/{}/Messages.json",
            self.api_base, self.config.account_sid
        );
        let response = self
            .http
            .post(url)
            .basic_auth(
                &self.config.account_sid,
                Some(self.config.auth_token.expose()),
            )
            .form(&[
                ("To", to.as_str()),
                ("From", from.as_str()),
                ("Body", &message.body),
            ])
            .send()
            .await
            .map_err(TextError::new)?;

        let status = response.status();
        if status.is_success() {
            Ok(())
        } else {
            // Twilio's error echoes the recipient; the log gets it masked.
            let body = response
                .text()
                .await
                .unwrap_or_default()
                .replace(message.to.as_str(), &message.to.masked());
            Err(TextError::new(TwilioRejected { status, body }))
        }
    }
}

impl TextSender for TwilioTextSender {
    fn channels(&self) -> &[CodeChannel] {
        &self.channels
    }

    fn send(&self, message: TextMessage) -> TextFuture<'_> {
        Box::pin(self.inner.deliver(message))
    }
}
