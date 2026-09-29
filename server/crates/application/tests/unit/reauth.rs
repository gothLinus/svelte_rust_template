use application::{
    account::dto::{ChangeEmailRequest, DeleteAccountRequest},
    auth::{
        Authenticated,
        dto::{ReauthMethod, ReauthenticateRequest},
    },
};
use domain::{secret::Secret, session::REAUTH_WINDOW, user::UserId};
use time::Duration;

use crate::support::{Fixture, PASSWORD, code_in, secret};

async fn stale_user(fx: &Fixture, email: &str) -> (Authenticated, Secret) {
    let signed_in = fx.register(email).await;
    fx.verify(UserId::from_uuid(signed_in.me.user.id)).await;
    fx.clock.advance(REAUTH_WINDOW + Duration::seconds(1));
    let user = fx.authenticate(&signed_in.token).await;
    (user, signed_in.token)
}

#[tokio::test]
async fn signing_in_counts_as_recent_authentication() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    assert!(alice.actor.recently_authenticated);
    fx.services.mfa.start_totp(&alice.actor).await.unwrap();
}

#[tokio::test]
async fn sensitive_changes_need_a_recent_reauthentication() {
    let fx = Fixture::new();
    let (alice, token) = stale_user(&fx, "alice@example.com").await;
    let actor = &alice.actor;
    assert!(!actor.recently_authenticated);

    let refused = [
        fx.services.mfa.start_totp(actor).await.map(|_| ()),
        fx.services
            .mfa
            .regenerate_recovery_codes(actor)
            .await
            .map(|_| ()),
        fx.services
            .passkeys
            .registration_options(actor)
            .await
            .map(|_| ()),
        fx.services
            .oauth
            .start("test", Some(actor), None)
            .await
            .map(|_| ()),
        fx.services.oauth.unlink(actor, "test").await,
        fx.services.account.remove_phone(actor).await.map(|_| ()),
        fx.services
            .account
            .request_email_change(
                actor,
                ChangeEmailRequest {
                    email: "new@example.com".to_owned(),
                },
            )
            .await,
        fx.services
            .account
            .delete_account(actor, DeleteAccountRequest { password: None })
            .await,
    ];
    for result in refused {
        assert_eq!(result.unwrap_err().code(), "reauth_required");
    }

    let err = fx
        .services
        .reauth
        .reauthenticate(
            actor,
            ReauthenticateRequest::Password {
                password: secret("not it"),
            },
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "validation_failed");

    fx.services
        .reauth
        .reauthenticate(
            actor,
            ReauthenticateRequest::Password {
                password: secret(PASSWORD),
            },
        )
        .await
        .unwrap();
    let alice = fx.authenticate(&token).await;
    assert!(alice.actor.recently_authenticated);
    fx.services.mfa.start_totp(&alice.actor).await.unwrap();

    fx.clock.advance(REAUTH_WINDOW);
    let alice = fx.authenticate(&token).await;
    assert_eq!(
        fx.services
            .mfa
            .start_totp(&alice.actor)
            .await
            .unwrap_err()
            .code(),
        "reauth_required"
    );
}

#[tokio::test]
async fn an_emailed_code_reauthenticates() {
    let fx = Fixture::new();
    let (alice, token) = stale_user(&fx, "alice@example.com").await;

    let methods = fx.services.reauth.methods(&alice.actor).await.unwrap();
    assert_eq!(
        methods.methods,
        [ReauthMethod::Password, ReauthMethod::EmailCode]
    );

    fx.services
        .reauth
        .send_email_code(&alice.actor)
        .await
        .unwrap();
    let mail = fx.mail.last_to("alice@example.com").unwrap();
    assert_eq!(mail.template, "reauthentication_code");
    let code = code_in(&mail.body);

    let wrong = fx
        .services
        .reauth
        .reauthenticate(
            &alice.actor,
            ReauthenticateRequest::EmailCode {
                code: secret("000000"),
            },
        )
        .await;
    assert_eq!(wrong.unwrap_err().code(), "validation_failed");
    fx.services
        .reauth
        .reauthenticate(
            &alice.actor,
            ReauthenticateRequest::EmailCode {
                code: secret(&code),
            },
        )
        .await
        .unwrap();
    assert!(fx.authenticate(&token).await.actor.recently_authenticated);
}

#[tokio::test]
async fn a_password_in_the_request_confirms_deleting_the_account() {
    let fx = Fixture::new();
    let (alice, _) = stale_user(&fx, "alice@example.com").await;

    let err = fx
        .services
        .account
        .delete_account(
            &alice.actor,
            DeleteAccountRequest {
                password: Some(secret("wrong")),
            },
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "validation_failed");
    fx.services
        .account
        .delete_account(
            &alice.actor,
            DeleteAccountRequest {
                password: Some(secret(PASSWORD)),
            },
        )
        .await
        .unwrap();
    assert!(fx.db.with(|state| state.users.is_empty()));
}

#[tokio::test]
async fn an_authenticator_code_reauthenticates_once_and_only_when_right() {
    let fx = Fixture::new();
    let (alice, token) = stale_user(&fx, "alice@example.com").await;
    let stale = alice.actor;
    fx.enable_totp(&application::actor::Actor {
        recently_authenticated: true,
        ..stale.clone()
    })
    .await;
    let totp = |code: &str| ReauthenticateRequest::Totp { code: secret(code) };

    let replayed = fx.totp_now(stale.user_id);
    let err = fx
        .services
        .reauth
        .reauthenticate(&stale, totp(&replayed))
        .await
        .unwrap_err();
    assert_eq!(err.code(), "validation_failed");

    fx.clock.advance(Duration::seconds(30));
    let current = fx.totp_now(stale.user_id);
    let wrong = format!("{:06}", (current.parse::<u32>().unwrap() + 1) % 1_000_000);
    let err = fx
        .services
        .reauth
        .reauthenticate(&stale, totp(&wrong))
        .await
        .unwrap_err();
    let application::AppError::Validation(errors) = err else {
        panic!("a wrong code is a field error");
    };
    assert_eq!(errors.fields()[0].field, "code");

    assert!(!fx.authenticate(&token).await.actor.recently_authenticated);
    fx.services
        .reauth
        .reauthenticate(&stale, totp(&current))
        .await
        .unwrap();
    assert!(fx.authenticate(&token).await.actor.recently_authenticated);

    let err = fx
        .services
        .reauth
        .reauthenticate(&stale, totp(&current))
        .await
        .unwrap_err();
    assert_eq!(err.code(), "validation_failed");
}

#[tokio::test]
async fn without_an_authenticator_app_no_code_reauthenticates() {
    let fx = Fixture::new();
    let (alice, token) = stale_user(&fx, "alice@example.com").await;

    for code in ["000000", "123456"] {
        let err = fx
            .services
            .reauth
            .reauthenticate(
                &alice.actor,
                ReauthenticateRequest::Totp { code: secret(code) },
            )
            .await
            .unwrap_err();
        assert_eq!(err.code(), "validation_failed");
    }
    assert!(!fx.authenticate(&token).await.actor.recently_authenticated);
}
