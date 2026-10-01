//! Every account has a language of its own: mail and texts to it follow that language, not the
//! server's default, which stays for accounts without one.

use application::{
    account::dto::{AddPhoneRequest, SetLocaleRequest},
    auth::Registered,
    passwordless::dto::TextChannel,
};
use domain::{
    i18n::{Locale, Message, Translator},
    session::ClientInfo,
    user::UserId,
};

use crate::support::{Fixture, register_request};

fn german() -> Locale {
    Locale::parse("de").unwrap()
}

fn set(locale: Option<&str>) -> SetLocaleRequest {
    SetLocaleRequest {
        locale: locale.map(str::to_owned),
    }
}

#[tokio::test]
async fn an_account_registered_in_german_gets_german_mail() {
    let fx = Fixture::new();

    let Registered::SignedIn(signed_in) = fx
        .services
        .auth
        .register(
            register_request("alice@example.com"),
            ClientInfo::default(),
            Some(german()),
        )
        .await
        .unwrap()
    else {
        panic!("verification is not required");
    };

    assert_eq!(signed_in.me.user.locale.as_deref(), Some("de"));
    let mail = fx.mail.last_to("alice@example.com").unwrap();
    let subject = fx
        .catalog
        .translate(&german(), &Message::new("mail-verify-email-subject"));
    assert_eq!(mail.subject, subject);
    assert_ne!(subject, fx.text("mail-verify-email-subject"));
}

#[tokio::test]
async fn choosing_a_language_changes_the_mail_and_texts_that_follow() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;

    let me = fx
        .services
        .account
        .set_locale(&alice.actor, set(Some(" de ")))
        .await
        .unwrap();
    assert_eq!(me.user.locale.as_deref(), Some("de"));

    fx.services
        .account
        .request_password_change(&alice.actor)
        .await
        .unwrap();
    let mail = fx.mail.last_to("alice@example.com").unwrap();
    assert_eq!(
        mail.subject,
        fx.catalog
            .translate(&german(), &Message::new("mail-password-reset-subject"))
    );

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
        fx.catalog.translate(
            &german(),
            &Message::new("sms-phone-verification-code")
                .arg("code", format!("{} {}", &code[..3], &code[3..]))
                .arg("app", "Acme")
                .arg("minutes", 10)
        )
    );
}

#[tokio::test]
async fn security_notices_go_out_in_the_owners_language() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    fx.services
        .account
        .set_locale(&alice.actor, set(Some("de")))
        .await
        .unwrap();

    fx.enable_totp(&alice.actor).await;

    let mail = fx.mail.last_to("alice@example.com").unwrap();
    assert_eq!(mail.template, "security_notice");
    let what = fx
        .catalog
        .translate(&german(), &Message::new("notice-totp-added"));
    assert!(mail.body.contains(&what), "{}", mail.body);
}

#[tokio::test]
async fn only_languages_the_catalog_has_can_be_chosen() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;

    for tag in ["xx", "not a tag"] {
        let err = fx
            .services
            .account
            .set_locale(&alice.actor, set(Some(tag)))
            .await
            .unwrap_err();
        let application::AppError::Validation(errors) = err else {
            panic!("expected a validation error, got {err:?}");
        };
        assert_eq!(errors.fields()[0].field, "locale");
        assert_eq!(errors.fields()[0].code, "unsupported_locale");
    }

    for cleared in [None, Some("")] {
        let me = fx
            .services
            .account
            .set_locale(&alice.actor, set(cleared))
            .await
            .unwrap();
        assert!(me.user.locale.is_none());
    }
}

#[tokio::test]
async fn a_language_the_catalog_dropped_falls_back_to_the_default() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    let id: UserId = alice.actor.user_id;
    fx.db.with(|state| {
        let mut parts = crate::support::memory::user_parts(&state.users[&id]);
        parts.locale = Locale::parse("fr");
        state
            .users
            .insert(id, domain::user::User::from_parts(parts));
    });

    fx.services
        .account
        .request_password_change(&alice.actor)
        .await
        .unwrap();

    let mail = fx.mail.last_to("alice@example.com").unwrap();
    assert_eq!(mail.subject, fx.text("mail-password-reset-subject"));
}

#[tokio::test]
async fn built_in_roles_are_described_in_the_requested_language() {
    let fx = Fixture::new();
    let admin = fx.admin("admin@example.com").await;

    let roles = fx
        .services
        .admin
        .list_roles(&admin.actor, &german())
        .await
        .unwrap();

    let admin_role = roles.iter().find(|role| role.name == "admin").unwrap();
    assert_eq!(
        admin_role.description,
        fx.catalog
            .translate(&german(), &Message::new("role-admin-description"))
    );
    let english = fx
        .services
        .admin
        .list_roles(&admin.actor, &Locale::EN)
        .await
        .unwrap();
    let user_role = english.iter().find(|role| role.name == "user").unwrap();
    assert_eq!(user_role.description, fx.text("role-user-description"));
}

#[tokio::test]
async fn the_catalog_ships_german() {
    let fx = Fixture::new();
    assert!(fx.catalog.locales().contains(&german()));
}

#[tokio::test]
async fn registering_a_taken_address_tells_the_owner_in_their_language() {
    let fx = Fixture::with(|settings| settings.require_email_verification = true);
    fx.services
        .auth
        .register(
            register_request("alice@example.com"),
            ClientInfo::default(),
            Some(german()),
        )
        .await
        .unwrap();

    fx.services
        .auth
        .register(
            register_request("alice@example.com"),
            ClientInfo::default(),
            None,
        )
        .await
        .unwrap();

    let mail = fx.mail.last_to("alice@example.com").unwrap();
    assert_eq!(mail.template, "already_registered");
    assert_eq!(
        mail.subject,
        fx.catalog
            .translate(&german(), &Message::new("mail-already-registered-subject"))
    );
}
