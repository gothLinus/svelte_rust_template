//! The emails the application sends, and the links in them.
//!
//! Each function builds a plain-text [`Mail`] for one event; the use case that triggers it hands
//! it to the [`Mailer`] port through `send`. In production the mailer writes to the outbox for
//! durable delivery. The words are not here: subject and body are catalog messages
//! (`locales/<language>/server/mail.ftl`, ids `mail-<template>-subject` and `-body`), rendered by
//! a `Voice`.
//!
//! To add an email: a function here with a distinct `template` name (and `valid_for` when it
//! carries a link or code), its two messages in the catalog, and a call from the use case.

use std::sync::Arc;

use domain::{
    i18n::{Locale, Message, Translator},
    mail::{Mail, Mailer},
    secret::Secret,
    user::Email,
};
use time::Duration;

use crate::error::ErrorChain;

/// Absolute links into the frontend.
///
/// The paths must match the SvelteKit routes in `web/src/routes/(public)/`. Tokens go into the URL
/// fragment, which browsers never send to a server, so they stay out of access logs and `Referer`
/// headers.
#[derive(Debug, Clone)]
pub struct Links {
    base_url: Arc<str>,
}

impl Links {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').into(),
        }
    }

    pub fn verify_email(&self, token: &Secret) -> String {
        format!("{}/verify-email#token={}", self.base_url, token.expose())
    }

    pub fn reset_password(&self, token: &Secret) -> String {
        format!("{}/reset-password#token={}", self.base_url, token.expose())
    }

    pub fn forgot_password(&self) -> String {
        format!("{}/forgot-password", self.base_url)
    }

    pub fn magic_link(&self, token: &Secret) -> String {
        format!("{}/magic-link#token={}", self.base_url, token.expose())
    }

    pub fn cancel_email_change(&self, token: &Secret) -> String {
        format!(
            "{}/cancel-email-change#token={}",
            self.base_url,
            token.expose()
        )
    }

    pub fn confirm_email(&self, token: &Secret) -> String {
        format!("{}/confirm-email#token={}", self.base_url, token.expose())
    }

    pub fn security_settings(&self) -> String {
        format!("{}/settings/security", self.base_url)
    }

    pub fn origin(&self) -> &str {
        &self.base_url
    }

    pub fn host(&self) -> &str {
        let authority = self
            .base_url
            .split_once("://")
            .map_or(&*self.base_url, |(_, rest)| rest);
        if authority.starts_with('[') {
            return authority
                .split_once(']')
                .map_or(authority, |(host, _)| &host[1..]);
        }
        authority
            .rsplit_once(':')
            .map_or(authority, |(host, _)| host)
    }
}

/// Hands mail to the transport. Failures are logged and swallowed: an email that could not be
/// queued must not fail the request that triggered it, nor tell the client whether an account
/// exists.
pub(crate) async fn send(mailer: &dyn Mailer, mail: Mail) {
    let to = mail.to.masked();
    let template = mail.template.clone();
    match mailer.send(mail).await {
        Ok(()) => tracing::debug!(to, template, "mail handed to transport"),
        Err(err) => {
            tracing::error!(to, template, error = %ErrorChain(&err), "failed to send mail");
        }
    }
}

/// Says things in one language: renders messages for mail and texts.
///
/// Users have no language setting yet, so this is always
/// [`Settings::locale`](crate::Settings::locale); when accounts get one, the use cases pick the
/// recipient's locale here and nothing else changes.
pub(crate) struct Voice<'a> {
    translator: &'a dyn Translator,
    locale: &'a Locale,
}

impl<'a> Voice<'a> {
    pub(crate) fn new(translator: &'a dyn Translator, locale: &'a Locale) -> Self {
        Self { translator, locale }
    }

    pub(crate) fn say(&self, message: &Message) -> String {
        self.translator.translate(self.locale, message)
    }
}

pub(crate) async fn notify<A: crate::Adapters>(ctx: &crate::Context<A>, to: Email, what: Message) {
    let link = ctx.settings.links.security_settings();
    send(
        &*ctx.mailer,
        security_notice(&ctx.voice(), to, &what, &link),
    )
    .await;
}

pub(crate) fn verify_email(voice: &Voice, to: Email, link: &str, ttl: Duration) -> Mail {
    Mail {
        to,
        template: "verify_email".to_owned(),
        valid_for: Some(ttl),
        subject: voice.say(&Message::new("mail-verify-email-subject")),
        body: voice.say(
            &Message::new("mail-verify-email-body")
                .arg("link", link)
                .arg("hours", ttl.whole_hours()),
        ),
    }
}

pub(crate) fn password_reset(voice: &Voice, to: Email, link: &str, ttl: Duration) -> Mail {
    Mail {
        to,
        template: "password_reset".to_owned(),
        valid_for: Some(ttl),
        subject: voice.say(&Message::new("mail-password-reset-subject")),
        body: voice.say(
            &Message::new("mail-password-reset-body")
                .arg("link", link)
                .arg("minutes", ttl.whole_minutes()),
        ),
    }
}

/// After the first proof of the address removed a password the owner may not have chosen: the
/// account was registered elsewhere, or by someone else.
pub(crate) fn choose_password(voice: &Voice, to: Email, link: &str, ttl: Duration) -> Mail {
    Mail {
        to,
        template: "choose_password".to_owned(),
        valid_for: Some(ttl),
        subject: voice.say(&Message::new("mail-choose-password-subject")),
        body: voice.say(
            &Message::new("mail-choose-password-body")
                .arg("link", link)
                .arg("minutes", ttl.whole_minutes()),
        ),
    }
}

pub(crate) fn password_changed(voice: &Voice, to: Email, forgot_link: &str) -> Mail {
    Mail {
        to,
        template: "password_changed".to_owned(),
        valid_for: None,
        subject: voice.say(&Message::new("mail-password-changed-subject")),
        body: voice.say(&Message::new("mail-password-changed-body").arg("link", forgot_link)),
    }
}

pub(crate) fn sign_in_code(
    voice: &Voice,
    to: Email,
    link: &str,
    code: &str,
    ttl: Duration,
) -> Mail {
    Mail {
        to,
        template: "sign_in_code".to_owned(),
        valid_for: Some(ttl),
        subject: voice.say(&Message::new("mail-sign-in-code-subject")),
        body: voice.say(
            &Message::new("mail-sign-in-code-body")
                .arg("code", code)
                .arg("link", link)
                .arg("minutes", ttl.whole_minutes()),
        ),
    }
}

pub(crate) fn already_registered(voice: &Voice, to: Email, forgot_link: &str) -> Mail {
    Mail {
        to,
        template: "already_registered".to_owned(),
        valid_for: None,
        subject: voice.say(&Message::new("mail-already-registered-subject")),
        body: voice.say(&Message::new("mail-already-registered-body").arg("link", forgot_link)),
    }
}

pub(crate) fn reauthentication_code(voice: &Voice, to: Email, code: &str) -> Mail {
    let ttl = domain::one_time_code::CODE_TTL;
    Mail {
        to,
        template: "reauthentication_code".to_owned(),
        valid_for: Some(ttl),
        subject: voice.say(&Message::new("mail-reauthentication-code-subject")),
        body: voice.say(
            &Message::new("mail-reauthentication-code-body")
                .arg("code", code)
                .arg("minutes", ttl.whole_minutes()),
        ),
    }
}

pub(crate) fn email_change_requested(
    voice: &Voice,
    to: Email,
    new_email: &str,
    cancel_link: &str,
) -> Mail {
    Mail {
        to,
        template: "email_change_requested".to_owned(),
        valid_for: None,
        subject: voice.say(&Message::new("mail-email-change-requested-subject")),
        body: voice.say(
            &Message::new("mail-email-change-requested-body")
                .arg("email", new_email)
                .arg("link", cancel_link),
        ),
    }
}

pub(crate) fn email_change_cancelled(voice: &Voice, to: Email, forgot_link: &str) -> Mail {
    Mail {
        to,
        template: "email_change_cancelled".to_owned(),
        valid_for: None,
        subject: voice.say(&Message::new("mail-email-change-cancelled-subject")),
        body: voice.say(&Message::new("mail-email-change-cancelled-body").arg("link", forgot_link)),
    }
}

pub(crate) fn confirm_email_change(voice: &Voice, to: Email, link: &str, ttl: Duration) -> Mail {
    Mail {
        to,
        template: "confirm_email_change".to_owned(),
        valid_for: Some(ttl),
        subject: voice.say(&Message::new("mail-confirm-email-change-subject")),
        body: voice.say(
            &Message::new("mail-confirm-email-change-body")
                .arg("link", link)
                .arg("hours", ttl.whole_hours()),
        ),
    }
}

/// Tells the account's owner about a change they may not have made themselves. `what` is a
/// sentence of its own (`notice-*` in the catalog), rendered first and set into the mail as a
/// whole.
pub(crate) fn security_notice(
    voice: &Voice,
    to: Email,
    what: &Message,
    settings_link: &str,
) -> Mail {
    Mail {
        to,
        template: "security_notice".to_owned(),
        valid_for: None,
        subject: voice.say(&Message::new("mail-security-notice-subject")),
        body: voice.say(
            &Message::new("mail-security-notice-body")
                .arg("what", voice.say(what))
                .arg("link", settings_link),
        ),
    }
}
