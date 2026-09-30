//! Mail and texts are worded by the catalog: subject and body come from
//! `locales/<language>/server`, in the language `Settings::locale` names, with the values (links,
//! codes, durations) set into the placeholders.

use std::sync::Arc;

use application::{
    account::dto::AddPhoneRequest,
    passwordless::dto::{EmailCodeRequest, PhoneCodeRequest, TextChannel},
};
use domain::i18n::{Locale, Message, Translator};
use i18n::Catalog;

use crate::support::{APP_URL, Fixture, code_in};

fn german() -> Arc<Catalog> {
    Arc::new(
        Catalog::embedded_with(&[(
            "de",
            vec![(
                "test.ftl",
                "mail-verify-email-subject = E-Mail-Adresse bestätigen\n\
                 mail-verify-email-body = Öffne { $link } innerhalb von { $hours ->\n\
                 \x20   [one] { $hours } Stunde\n\
                 \x20  *[other] { $hours } Stunden\n\
                 }.\n\
                 sms-sign-in-code = { $code } ist dein { $app }-Code ({ $minutes } Minuten).\n",
            )],
        )])
        .unwrap(),
    )
}

#[tokio::test]
async fn the_verification_mail_is_the_catalog_text_with_its_values() {
    let fx = Fixture::new();
    fx.register("alice@example.com").await;

    let mail = fx.mail.last_to("alice@example.com").unwrap();
    let token = fx.mail.token_for("alice@example.com");
    let link = format!("{APP_URL}/verify-email#token={}", token.expose());
    assert_eq!(mail.template, "verify_email");
    assert_eq!(mail.subject, fx.text("mail-verify-email-subject"));
    assert_eq!(
        mail.body,
        fx.say(
            &Message::new("mail-verify-email-body")
                .arg("link", link.as_str())
                .arg("hours", 24)
        )
    );
    // The link is on a line of its own, so mail clients make it clickable.
    assert!(mail.body.lines().any(|line| line == link));
}

#[tokio::test]
async fn durations_follow_the_token_policy_and_use_plural_forms() {
    let fx = Fixture::with(|settings| {
        settings.tokens.email_verification_ttl = time::Duration::hours(1);
    });
    fx.register("alice@example.com").await;

    let mail = fx.mail.last_to("alice@example.com").unwrap();
    let hours = |count: i32| {
        fx.say(
            &Message::new("mail-verify-email-body")
                .arg("link", "")
                .arg("hours", count),
        )
    };
    // One hour, and the catalog decides how that reads.
    assert!(mail.body.contains("1 hour"), "{}", mail.body);
    assert!(hours(1).contains("1 hour") && !hours(1).contains("1 hours"));
    assert!(hours(3).contains("3 hours"));
}

#[tokio::test]
async fn the_sign_in_code_mail_holds_the_code_and_the_link() {
    let fx = Fixture::new();
    fx.register("alice@example.com").await;

    fx.services
        .passwordless
        .request_email_code(EmailCodeRequest {
            email: "alice@example.com".to_owned(),
        })
        .await
        .unwrap();

    let mail = fx.mail.last_to("alice@example.com").unwrap();
    assert_eq!(mail.template, "sign_in_code");
    assert_eq!(mail.subject, fx.text("mail-sign-in-code-subject"));
    let code = code_in(&mail.body);
    let token = fx.mail.token_for("alice@example.com");
    let link = format!("{APP_URL}/magic-link#token={}", token.expose());
    assert_eq!(
        mail.body,
        fx.say(
            &Message::new("mail-sign-in-code-body")
                .arg("code", format!("{} {}", &code[..3], &code[3..]))
                .arg("link", link)
                .arg("minutes", 15)
        )
    );
}

#[tokio::test]
async fn texts_are_the_catalog_text_with_the_code() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;

    fx.services
        .account
        .add_phone(
            &alice.actor,
            AddPhoneRequest {
                phone: "+49 170 1234567".to_owned(),
                channel: TextChannel::Sms,
            },
        )
        .await
        .unwrap();

    let text = fx.texts.sent().pop().unwrap();
    let code = fx.texts.last_code_to("+491701234567");
    assert_eq!(
        text.body,
        fx.say(
            &Message::new("sms-phone-verification-code")
                .arg("code", format!("{} {}", &code[..3], &code[3..]))
                .arg("app", "Acme")
                .arg("minutes", 10)
        )
    );
}

#[tokio::test]
async fn sign_in_texts_are_worded_by_the_catalog_too() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    fx.services
        .account
        .add_phone(
            &alice.actor,
            AddPhoneRequest {
                phone: "+49 170 1234567".to_owned(),
                channel: TextChannel::Sms,
            },
        )
        .await
        .unwrap();
    let code = fx.texts.last_code_to("+491701234567");
    fx.services
        .account
        .verify_phone(
            &alice.actor,
            application::mfa::dto::CodeRequest {
                code: crate::support::secret(&code),
            },
        )
        .await
        .unwrap();

    fx.services
        .passwordless
        .request_phone_code(PhoneCodeRequest {
            phone: "+49 170 1234567".to_owned(),
            channel: TextChannel::Sms,
        })
        .await
        .unwrap();

    let text = fx.texts.sent().pop().unwrap();
    let code = fx.texts.last_code_to("+491701234567");
    assert_eq!(
        text.body,
        fx.say(
            &Message::new("sms-sign-in-code")
                .arg("code", format!("{} {}", &code[..3], &code[3..]))
                .arg("app", "Acme")
                .arg("minutes", 10)
        )
    );
}

#[tokio::test]
async fn security_notices_embed_their_own_sentence() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;

    fx.enable_totp(&alice.actor).await;

    let notice = fx.mail.last_to("alice@example.com").unwrap();
    assert_eq!(notice.template, "security_notice");
    assert_eq!(notice.subject, fx.text("mail-security-notice-subject"));
    assert!(
        notice.body.starts_with(&fx.text("notice-totp-added")),
        "{}",
        notice.body
    );
    assert!(
        notice
            .body
            .contains(&format!("{APP_URL}/settings/security"))
    );
}

#[tokio::test]
async fn mail_and_texts_follow_the_configured_language() {
    let fx = Fixture::with_catalog(german(), |settings| {
        settings.locale = Locale::parse("de").unwrap();
    });
    fx.register("alice@example.com").await;

    let mail = fx.mail.last_to("alice@example.com").unwrap();
    assert_eq!(mail.subject, "E-Mail-Adresse bestätigen");
    assert!(mail.body.starts_with("Öffne http"), "{}", mail.body);
    assert!(
        mail.body.ends_with("innerhalb von 24 Stunden."),
        "{}",
        mail.body
    );
    assert_eq!(
        fx.catalog.translate(
            &Locale::parse("de").unwrap(),
            &Message::new("mail-password-reset-subject")
        ),
        fx.text("mail-password-reset-subject")
    );
}
