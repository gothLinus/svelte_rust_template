use application::{
    AppError,
    account::dto::{
        AddPhoneRequest, ChangeEmailRequest, DeleteAccountRequest, UpdateProfileRequest,
    },
    auth::{
        LoginOutcome,
        dto::{ConfirmEmailRequest, LoginRequest},
    },
    dto::{MfaMethod, SecretInput},
    mfa::dto::CodeRequest,
    oauth::{CallbackParams, OAuthOutcome},
    passkeys::dto::{PasskeyAssertionRequest, RegisterPasskeyRequest},
    passwordless::dto::{
        EmailCodeRequest, MagicLinkRequest, PhoneCodeRequest, TextChannel, VerifyEmailCodeRequest,
        VerifyPhoneCodeRequest,
    },
};
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use domain::{identity::ProviderProfile, secret::Secret, session::ClientInfo, user::UserId};
use time::Duration;

use crate::support::{
    Fixture, Outcome, PASSWORD, code_in,
    fakes::{FakeCrypto, fake_digest},
    register_request, secret,
};

fn login(identifier: &str) -> LoginRequest {
    LoginRequest {
        identifier: identifier.to_owned(),
        password: secret(PASSWORD),
    }
}

fn code(value: &str) -> CodeRequest {
    CodeRequest {
        code: secret(value),
    }
}

fn totp_now(fx: &Fixture, user: UserId) -> String {
    fx.totp_now(user)
}

async fn enable_totp(fx: &Fixture, actor: &application::actor::Actor) -> Vec<String> {
    fx.enable_totp(actor).await
}

#[tokio::test]
async fn usernames_and_verified_phone_numbers_sign_in_like_emails() {
    let fx = Fixture::new();
    let mut request = register_request("alice@example.com");
    request.username = "Alice".to_owned();
    fx.services
        .auth
        .register(request, ClientInfo::default())
        .await
        .unwrap();

    let me = fx
        .services
        .auth
        .login(login("ALICE"), ClientInfo::default(), None)
        .await
        .unwrap()
        .signed_in()
        .me;
    assert_eq!(me.user.username, "alice");

    let mut taken = register_request("bob@example.com");
    taken.username = "alice".to_owned();
    let err = fx
        .services
        .auth
        .register(taken, ClientInfo::default())
        .await
        .unwrap_err();
    let AppError::Validation(errors) = err else {
        panic!("expected a field error")
    };
    assert_eq!(errors.fields()[0].code, "username_taken");

    let session = fx
        .services
        .auth
        .login(login("alice"), ClientInfo::default(), None)
        .await
        .unwrap()
        .signed_in()
        .token;
    fx.verify(fx.authenticate(&session).await.actor.user_id)
        .await;
    let alice = fx.authenticate(&session).await;
    fx.db.with(|state| {
        let phone = domain::user::PhoneNumber::parse("+491701234567").unwrap();
        let user = state.users.get(&alice.actor.user_id).unwrap().clone();
        let mut parts = parts_of(&user);
        parts.phone = Some(phone);
        state
            .users
            .insert(user.id(), domain::user::User::from_parts(parts));
    });
    let err = fx
        .services
        .auth
        .login(login("+49 170 1234567"), ClientInfo::default(), None)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "invalid_credentials");

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
    let sent = fx.texts.last_code_to("+491701234567");
    let me = fx
        .services
        .account
        .verify_phone(&alice.actor, code(&sent))
        .await
        .unwrap();
    assert!(me.user.phone_verified);
    fx.services
        .auth
        .login(login("+491701234567"), ClientInfo::default(), None)
        .await
        .unwrap()
        .signed_in();
}

fn parts_of(user: &domain::user::User) -> domain::user::UserParts {
    domain::user::UserParts {
        id: user.id(),
        email: user.email().clone(),
        username: user.username().clone(),
        phone: user.phone().cloned(),
        phone_verified_at: user.phone_verified_at(),
        password_hash: user.password_hash().cloned(),
        email_verified_at: user.email_verified_at(),
        disabled_at: user.disabled_at(),
        created_at: user.created_at(),
        updated_at: user.updated_at(),
    }
}

#[tokio::test]
async fn profile_updates_change_the_username() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    fx.user("bob@example.com").await;

    let update = |username: &str| UpdateProfileRequest {
        username: username.to_owned(),
    };
    let me = fx
        .services
        .account
        .update_profile(&alice.actor, update("Alice.L"))
        .await
        .unwrap();
    assert_eq!(me.user.username, "alice.l");

    for (username, code) in [("Bob", "username_taken"), ("", "required")] {
        let AppError::Validation(errors) = fx
            .services
            .account
            .update_profile(&alice.actor, update(username))
            .await
            .unwrap_err()
        else {
            panic!("expected a field error")
        };
        assert_eq!(errors.fields()[0].code, code, "{username}");
    }
}

#[tokio::test]
async fn an_emailed_code_signs_in_once_and_verifies_the_address() {
    let fx = Fixture::new();
    fx.register("alice@example.com").await;

    fx.services
        .passwordless
        .request_email_code(EmailCodeRequest {
            email: "Alice@example.com".to_owned(),
        })
        .await
        .unwrap();
    let mail = fx.mail.last_to("alice@example.com").unwrap();
    // The code stays out of the subject, which mail logs and notifications show.
    assert_eq!(mail.subject, fx.text("mail-sign-in-code-subject"));
    assert_eq!(mail.template, "sign_in_code");
    let sent = code_in(&mail.body);

    let verify = |value: &str| VerifyEmailCodeRequest {
        email: "alice@example.com".to_owned(),
        code: secret(value),
    };
    let err = fx
        .services
        .passwordless
        .verify_email_code(verify("000000"), ClientInfo::default(), None)
        .await
        .unwrap_err();
    crate::support::assert_invalid_code(&err);

    let signed_in = fx
        .services
        .passwordless
        .verify_email_code(
            verify(&format!("{} {}", &sent[..3], &sent[3..])),
            ClientInfo::default(),
            None,
        )
        .await
        .unwrap()
        .signed_in();
    assert!(signed_in.me.user.email_verified);

    let again = fx
        .services
        .passwordless
        .verify_email_code(verify(&sent), ClientInfo::default(), None)
        .await
        .unwrap_err();
    crate::support::assert_invalid_code(&again);
    let link = fx
        .services
        .passwordless
        .magic_link(
            MagicLinkRequest {
                token: SecretInput(fx.mail.token_for("alice@example.com")),
            },
            ClientInfo::default(),
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(link.code(), "invalid_token");
}

#[tokio::test]
async fn codes_stop_working_after_too_many_guesses_and_unknown_addresses_get_no_mail() {
    let fx = Fixture::new();
    fx.register("alice@example.com").await;
    fx.services
        .passwordless
        .request_email_code(EmailCodeRequest {
            email: "nobody@example.com".to_owned(),
        })
        .await
        .unwrap();
    assert!(fx.mail.last_to("nobody@example.com").is_none());

    fx.services
        .passwordless
        .request_email_code(EmailCodeRequest {
            email: "alice@example.com".to_owned(),
        })
        .await
        .unwrap();
    let right = fx.db.with(|state| state.codes[0].clone());
    for _ in 0..5 {
        fx.services
            .passwordless
            .verify_email_code(
                VerifyEmailCodeRequest {
                    email: "alice@example.com".to_owned(),
                    code: secret("999999"),
                },
                ClientInfo::default(),
                None,
            )
            .await
            .unwrap_err();
    }
    assert_eq!(fx.db.with(|state| state.codes[0].attempts), 5);
    assert_eq!(right.attempts, 0);
}

#[tokio::test]
async fn a_magic_link_signs_in() {
    let fx = Fixture::new();
    fx.register("alice@example.com").await;
    fx.services
        .passwordless
        .request_email_code(EmailCodeRequest {
            email: "alice@example.com".to_owned(),
        })
        .await
        .unwrap();

    let signed_in = fx
        .services
        .passwordless
        .magic_link(
            MagicLinkRequest {
                token: SecretInput(fx.mail.token_for("alice@example.com")),
            },
            ClientInfo::default(),
            None,
        )
        .await
        .unwrap()
        .signed_in();
    assert_eq!(signed_in.me.user.email, "alice@example.com");
    assert!(fx.db.with(|state| state.codes.is_empty()));
}

#[tokio::test]
async fn a_texted_code_signs_in_with_a_verified_number() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    fx.services
        .account
        .add_phone(
            &alice.actor,
            AddPhoneRequest {
                phone: "+15551234567".to_owned(),
                channel: TextChannel::Whatsapp,
            },
        )
        .await
        .unwrap();
    let sent = fx.texts.last_code_to("+15551234567");
    fx.services
        .account
        .verify_phone(&alice.actor, code(&sent))
        .await
        .unwrap();

    fx.services
        .passwordless
        .request_phone_code(PhoneCodeRequest {
            phone: "+1 555 123 4567".to_owned(),
            channel: TextChannel::Sms,
        })
        .await
        .unwrap();
    let sent = fx.texts.last_code_to("+15551234567");
    let signed_in = fx
        .services
        .passwordless
        .verify_phone_code(
            VerifyPhoneCodeRequest {
                phone: "+15551234567".to_owned(),
                code: secret(&sent),
            },
            ClientInfo::default(),
            None,
        )
        .await
        .unwrap()
        .signed_in();
    assert_eq!(signed_in.me.user.phone.as_deref(), Some("+15551234567"));

    let before = fx.texts.sent().len();
    fx.services
        .passwordless
        .request_phone_code(PhoneCodeRequest {
            phone: "+15550000000".to_owned(),
            channel: TextChannel::Sms,
        })
        .await
        .unwrap();
    assert_eq!(fx.texts.sent().len(), before);

    let me = fx
        .services
        .account
        .remove_phone(&alice.actor)
        .await
        .unwrap();
    assert_eq!(me.user.phone, None);
}

#[tokio::test]
async fn changing_the_email_needs_the_link_sent_to_the_new_address() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    fx.register("taken@example.com").await;

    let err = fx
        .services
        .account
        .request_email_change(
            &alice.actor,
            ChangeEmailRequest {
                email: "taken@example.com".to_owned(),
            },
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "email_taken");

    fx.services
        .account
        .request_email_change(
            &alice.actor,
            ChangeEmailRequest {
                email: "New@Example.com".to_owned(),
            },
        )
        .await
        .unwrap();
    let mail = fx.mail.last_to("new@example.com").unwrap();
    assert_eq!(mail.subject, fx.text("mail-confirm-email-change-subject"));
    let me = fx.services.account.me(&alice.user).await.unwrap();
    assert_eq!(me.user.email, "alice@example.com");

    fx.services
        .auth
        .confirm_email_change(
            ConfirmEmailRequest {
                token: SecretInput(fx.mail.token_for("new@example.com")),
            },
            None,
        )
        .await
        .unwrap();
    let user = fx
        .db
        .with(|state| state.users[&alice.actor.user_id].clone());
    assert_eq!(user.email().as_str(), "new@example.com");
    assert!(user.is_email_verified());
    let notice = fx.mail.last_to("alice@example.com").unwrap();
    assert_eq!(notice.subject, fx.text("mail-security-notice-subject"));
}

#[tokio::test]
async fn an_authenticator_app_adds_a_second_step() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    let recovery = enable_totp(&fx, &alice.actor).await;
    assert_eq!(recovery.len(), 10);

    let LoginOutcome::MfaRequired(required) = fx
        .services
        .auth
        .login(login("alice@example.com"), ClientInfo::default(), None)
        .await
        .unwrap()
    else {
        panic!("expected a second step")
    };
    assert_eq!(
        required.challenge.methods,
        [MfaMethod::Totp, MfaMethod::RecoveryCode]
    );
    let pending = fx.services.mfa.pending(&required.token).await.unwrap();
    assert_eq!(pending.methods, required.challenge.methods);

    let replayed = totp_now(&fx, alice.actor.user_id);
    let err = fx
        .services
        .mfa
        .complete_with_totp(
            &required.token,
            code(&replayed),
            ClientInfo::default(),
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "validation_failed");

    fx.clock.advance(Duration::seconds(30));
    let current = totp_now(&fx, alice.actor.user_id);
    let signed_in = fx
        .services
        .mfa
        .complete_with_totp(&required.token, code(&current), ClientInfo::default(), None)
        .await
        .unwrap();
    assert_eq!(signed_in.me.user.email, "alice@example.com");

    let err = fx.services.mfa.pending(&required.token).await.unwrap_err();
    assert_eq!(err.code(), "mfa_expired");

    let required = fx
        .services
        .auth
        .login(login("alice@example.com"), ClientInfo::default(), None)
        .await
        .unwrap()
        .mfa_required();
    fx.services
        .mfa
        .complete_with_recovery_code(
            &required.token,
            code(&recovery[0].to_uppercase()),
            ClientInfo::default(),
            None,
        )
        .await
        .unwrap();
    let required = fx
        .services
        .auth
        .login(login("alice@example.com"), ClientInfo::default(), None)
        .await
        .unwrap()
        .mfa_required();
    let err = fx
        .services
        .mfa
        .complete_with_recovery_code(
            &required.token,
            code(&recovery[0]),
            ClientInfo::default(),
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "validation_failed");

    fx.clock.advance(Duration::seconds(30));
    let err = fx
        .services
        .mfa
        .remove_totp(&alice.actor, code("000000"))
        .await
        .unwrap_err();
    assert_eq!(err.code(), "validation_failed");
    let current = totp_now(&fx, alice.actor.user_id);
    fx.services
        .mfa
        .remove_totp(&alice.actor, code(&current))
        .await
        .unwrap();
    fx.services
        .auth
        .login(login("alice@example.com"), ClientInfo::default(), None)
        .await
        .unwrap()
        .signed_in();
    assert!(fx.db.with(|state| state.recovery_codes.is_empty()));
}

#[tokio::test]
async fn the_second_step_gives_up_after_too_many_wrong_codes() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    enable_totp(&fx, &alice.actor).await;
    let required = fx
        .services
        .auth
        .login(login("alice@example.com"), ClientInfo::default(), None)
        .await
        .unwrap()
        .mfa_required();

    for _ in 0..5 {
        fx.services
            .mfa
            .complete_with_totp(&required.token, code("000000"), ClientInfo::default(), None)
            .await
            .unwrap_err();
    }
    fx.clock.advance(Duration::seconds(30));
    let current = totp_now(&fx, alice.actor.user_id);
    let err = fx
        .services
        .mfa
        .complete_with_totp(&required.token, code(&current), ClientInfo::default(), None)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "mfa_expired");
}

#[tokio::test]
async fn recovery_codes_can_be_regenerated_only_with_a_second_step() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    let err = fx
        .services
        .mfa
        .regenerate_recovery_codes(&alice.actor)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "mfa_disabled");

    let first = enable_totp(&fx, &alice.actor).await;
    let second = fx
        .services
        .mfa
        .regenerate_recovery_codes(&alice.actor)
        .await
        .unwrap()
        .codes;
    assert_eq!(second.len(), 10);
    assert_ne!(first, second);

    let err = fx.services.mfa.start_totp(&alice.actor).await.unwrap_err();
    assert_eq!(err.code(), "totp_enabled");
}

const ORIGIN: &str = "https://app.test";

fn b64(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

fn authenticator_data(flags: u8, sign_count: u32, credential: Option<&[u8]>) -> Vec<u8> {
    let mut data = fake_digest(b"app.test").to_vec();
    data.push(flags);
    data.extend_from_slice(&sign_count.to_be_bytes());
    if let Some(id) = credential {
        data.extend_from_slice(&[0; 16]);
        data.extend_from_slice(&u16::try_from(id.len()).unwrap().to_be_bytes());
        data.extend_from_slice(id);
    }
    data
}

fn client_data(kind: &str, challenge: &[u8]) -> Vec<u8> {
    let challenge = b64(challenge);
    format!(r#"{{"type":"{kind}","challenge":"{challenge}","origin":"{ORIGIN}"}}"#).into_bytes()
}

async fn register_passkey(
    fx: &Fixture,
    actor: &application::actor::Actor,
    credential: &[u8],
) -> Option<Vec<String>> {
    let options = fx
        .services
        .passkeys
        .registration_options(actor)
        .await
        .unwrap();
    assert_eq!(options.public_key.rp.id, "app.test");
    fx.services
        .passkeys
        .register(
            actor,
            RegisterPasskeyRequest {
                challenge_id: options.challenge_id,
                name: "MacBook".to_owned(),
                credential_id: credential.to_vec(),
                client_data_json: client_data("webauthn.create", &options.public_key.challenge),
                authenticator_data: authenticator_data(0x45, 0, Some(credential)),
                public_key: b"pk-alice".to_vec(),
                public_key_algorithm: -7,
                transports: vec!["internal".to_owned()],
            },
        )
        .await
        .unwrap()
        .recovery_codes
}

fn assertion(
    challenge_id: uuid::Uuid,
    challenge: &[u8],
    credential: &[u8],
    flags: u8,
    sign_count: u32,
) -> PasskeyAssertionRequest {
    let data = authenticator_data(flags, sign_count, None);
    let client = client_data("webauthn.get", challenge);
    let mut message = data.clone();
    message.extend_from_slice(&fake_digest(&client));
    PasskeyAssertionRequest {
        challenge_id,
        credential_id: credential.to_vec(),
        client_data_json: client,
        authenticator_data: data,
        signature: FakeCrypto::sign(b"pk-alice", &message),
        user_handle: None,
    }
}

#[tokio::test]
async fn passkeys_sign_in_alone_and_serve_as_the_second_step() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    let recovery = register_passkey(&fx, &alice.actor, b"cred-1").await;
    assert_eq!(recovery.map(|codes| codes.len()), Some(10));
    assert_eq!(
        fx.services.passkeys.list(&alice.actor).await.unwrap().len(),
        1
    );

    let options = fx.services.passkeys.login_options().await.unwrap();
    let err = fx
        .services
        .passkeys
        .login(
            assertion(
                options.challenge_id,
                &options.public_key.challenge,
                b"cred-1",
                0x01,
                1,
            ),
            ClientInfo::default(),
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "invalid_passkey");
    let options = fx.services.passkeys.login_options().await.unwrap();
    let signed_in = fx
        .services
        .passkeys
        .login(
            assertion(
                options.challenge_id,
                &options.public_key.challenge,
                b"cred-1",
                0x05,
                1,
            ),
            ClientInfo::default(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(signed_in.me.user.email, "alice@example.com");

    let err = fx
        .services
        .passkeys
        .login(
            assertion(
                options.challenge_id,
                &options.public_key.challenge,
                b"cred-1",
                0x05,
                2,
            ),
            ClientInfo::default(),
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "invalid_passkey");
}

#[tokio::test]
async fn a_passkey_serves_as_the_second_step_until_it_is_removed() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    register_passkey(&fx, &alice.actor, b"cred-1").await;

    let required = fx
        .services
        .auth
        .login(login("alice@example.com"), ClientInfo::default(), None)
        .await
        .unwrap()
        .mfa_required();
    assert_eq!(
        required.challenge.methods,
        [MfaMethod::Passkey, MfaMethod::RecoveryCode]
    );
    let options = fx
        .services
        .passkeys
        .second_step_options(&required.token)
        .await
        .unwrap();
    assert_eq!(options.public_key.allow_credentials.len(), 1);
    fx.services
        .passkeys
        .complete_second_step(
            &required.token,
            assertion(
                options.challenge_id,
                &options.public_key.challenge,
                b"cred-1",
                0x01,
                2,
            ),
            ClientInfo::default(),
            None,
        )
        .await
        .unwrap();

    let id = domain::passkey::PasskeyId::from_uuid(
        fx.services.passkeys.list(&alice.actor).await.unwrap()[0].id,
    );
    fx.services.passkeys.delete(&alice.actor, id).await.unwrap();
    assert!(fx.db.with(|state| state.recovery_codes.is_empty()));
    fx.services
        .auth
        .login(login("alice@example.com"), ClientInfo::default(), None)
        .await
        .unwrap()
        .signed_in();
}

#[tokio::test]
async fn passkey_registration_rejects_forged_ceremonies() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    let options = fx
        .services
        .passkeys
        .registration_options(&alice.actor)
        .await
        .unwrap();

    let request = |client: Vec<u8>, credential: &[u8]| RegisterPasskeyRequest {
        challenge_id: options.challenge_id,
        name: "Key".to_owned(),
        credential_id: b"cred-1".to_vec(),
        client_data_json: client,
        authenticator_data: authenticator_data(0x41, 0, Some(credential)),
        public_key: b"pk-alice".to_vec(),
        public_key_algorithm: -7,
        transports: Vec::new(),
    };
    let wrong_origin = format!(
        r#"{{"type":"webauthn.create","challenge":"{}","origin":"https://evil.test"}}"#,
        b64(&options.public_key.challenge)
    )
    .into_bytes();
    let err = fx
        .services
        .passkeys
        .register(&alice.actor, request(wrong_origin, b"cred-1"))
        .await
        .unwrap_err();
    assert_eq!(err.code(), "invalid_passkey");

    let err = fx
        .services
        .passkeys
        .register(
            &alice.actor,
            request(
                client_data("webauthn.create", &options.public_key.challenge),
                b"cred-1",
            ),
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "invalid_passkey");
}

fn profile(subject: &str, email: &str, verified: bool) -> ProviderProfile {
    ProviderProfile {
        subject: subject.to_owned(),
        email: Some(email.to_owned()),
        email_verified: verified,
        name: Some("Carol Provider".to_owned()),
    }
}

async fn oauth_callback(
    fx: &Fixture,
    code_value: &str,
    linking: Option<&application::actor::Actor>,
) -> Result<application::oauth::OAuthCallback, AppError> {
    let started = fx
        .services
        .oauth
        .start("test", linking, Some("/notes"))
        .await
        .unwrap();
    assert!(started.url.starts_with("https://provider.test/"));
    let state = started.state.expose().to_owned();
    fx.services
        .oauth
        .callback(
            CallbackParams {
                provider: "test",
                code: Some(Secret::new(code_value)),
                state: &state,
                cookie_state: Some(&started.state),
                error: None,
            },
            linking,
            ClientInfo::default(),
            None,
        )
        .await
}

#[tokio::test]
async fn a_social_account_signs_up_and_then_signs_in() {
    let fx = Fixture::new();
    fx.providers.add_account(
        "code-carol",
        profile("sub-carol", "carol@example.com", true),
    );

    let done = oauth_callback(&fx, "code-carol", None).await.unwrap();
    assert_eq!(done.redirect_to, "/notes");
    let OAuthOutcome::SignedIn(signed_in) = done.outcome else {
        panic!("expected a session")
    };
    assert_eq!(signed_in.me.user.email, "carol@example.com");
    assert_eq!(signed_in.me.user.username, "carol");
    assert!(signed_in.me.user.email_verified);
    assert!(!signed_in.me.user.has_password);
    assert_eq!(signed_in.me.user.roles, ["user"]);

    let done = oauth_callback(&fx, "code-carol", None).await.unwrap();
    assert!(matches!(done.outcome, OAuthOutcome::SignedIn(_)));
    assert_eq!(fx.db.with(|state| state.users.len()), 1);

    let carol = fx
        .authenticate(&match oauth_callback(&fx, "code-carol", None)
            .await
            .unwrap()
            .outcome
        {
            OAuthOutcome::SignedIn(signed_in) => signed_in.token,
            _ => panic!("expected a session"),
        })
        .await;
    fx.services
        .account
        .delete_account(&carol.actor, DeleteAccountRequest { password: None })
        .await
        .unwrap();
}

#[tokio::test]
async fn a_social_sign_up_gets_a_free_username() {
    let fx = Fixture::new();
    fx.register("carol@elsewhere.com").await;
    fx.providers.add_account(
        "code-carol",
        profile("sub-carol", "carol@example.com", true),
    );

    let OAuthOutcome::SignedIn(signed_in) = oauth_callback(&fx, "code-carol", None)
        .await
        .unwrap()
        .outcome
    else {
        panic!("expected a session")
    };
    let username = signed_in.me.user.username;
    assert!(
        username.strip_prefix("carol").is_some_and(|digits| {
            digits.len() == 4 && digits.chars().all(|c| c.is_ascii_digit())
        }),
        "{username}"
    );
}

#[tokio::test]
async fn a_social_account_never_takes_over_an_existing_address() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    fx.providers
        .add_account("code-a", profile("sub-a", "alice@example.com", true));

    let err = oauth_callback(&fx, "code-a", None).await.unwrap_err();
    assert_eq!(err.code(), "email_in_use");

    let done = oauth_callback(&fx, "code-a", Some(&alice.actor))
        .await
        .unwrap();
    assert!(matches!(done.outcome, OAuthOutcome::Linked));
    let identities = fx.services.oauth.identities(&alice.actor).await.unwrap();
    assert_eq!(identities[0].provider_name, "Test");

    let done = oauth_callback(&fx, "code-a", None).await.unwrap();
    let OAuthOutcome::SignedIn(signed_in) = done.outcome else {
        panic!("expected a session")
    };
    assert_eq!(signed_in.me.user.email, "alice@example.com");

    let bob = fx.user("bob@example.com").await;
    let err = oauth_callback(&fx, "code-a", Some(&bob.actor))
        .await
        .unwrap_err();
    assert_eq!(err.code(), "identity_taken");

    fx.services
        .oauth
        .unlink(&alice.actor, "test")
        .await
        .unwrap();
    let err = fx
        .services
        .oauth
        .unlink(&alice.actor, "test")
        .await
        .unwrap_err();
    assert_eq!(err.code(), "not_found");
}

#[tokio::test]
async fn social_sign_in_checks_the_state_and_the_provider() {
    let fx = Fixture::new();
    fx.providers
        .add_account("code-c", profile("sub-c", "carol@example.com", true));

    let started = fx
        .services
        .oauth
        .start("test", None, Some("//evil.test"))
        .await
        .unwrap();
    let state = started.state.expose().to_owned();
    let other = Secret::new("another browser");
    let err = fx
        .services
        .oauth
        .callback(
            CallbackParams {
                provider: "test",
                code: Some(Secret::new("code-c")),
                state: &state,
                cookie_state: Some(&other),
                error: None,
            },
            None,
            ClientInfo::default(),
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "oauth_state_invalid");

    let err = fx
        .services
        .oauth
        .start("nope", None, None)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "not_found");

    fx.providers
        .fail
        .store(true, std::sync::atomic::Ordering::SeqCst);
    let err = oauth_callback(&fx, "code-c", None).await.unwrap_err();
    assert_eq!(err.code(), "provider_unavailable");
    fx.providers
        .fail
        .store(false, std::sync::atomic::Ordering::SeqCst);

    let started = fx
        .services
        .oauth
        .start("test", None, Some("//evil.test"))
        .await
        .unwrap();
    let state = started.state.expose().to_owned();
    let done = fx
        .services
        .oauth
        .callback(
            CallbackParams {
                provider: "test",
                code: Some(Secret::new("code-c")),
                state: &state,
                cookie_state: Some(&started.state),
                error: None,
            },
            None,
            ClientInfo::default(),
            None,
        )
        .await
        .unwrap();
    assert_eq!(done.redirect_to, "/dashboard");
    let OAuthOutcome::SignedIn(signed_in) = done.outcome else {
        panic!("expected a session")
    };
    assert!(signed_in.me.user.email_verified);
    assert!(fx.mail.last_to("carol@example.com").is_none());

    fx.providers
        .add_account("code-u", profile("sub-u", "victim@example.com", false));
    let err = oauth_callback(&fx, "code-u", None).await.unwrap_err();
    assert_eq!(err.code(), "email_unverified");
    assert!(fx.db.with(|state| {
        state
            .users
            .values()
            .all(|user| user.email().as_str() != "victim@example.com")
    }));
}

#[tokio::test]
async fn the_security_overview_lists_everything() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    enable_totp(&fx, &alice.actor).await;
    register_passkey(&fx, &alice.actor, b"cred-9").await;

    let overview = fx.services.account.security(&alice.actor).await.unwrap();
    assert!(overview.has_password);
    assert!(overview.mfa_enabled);
    assert!(overview.totp_enabled);
    assert_eq!(overview.recovery_codes_remaining, 10);
    assert_eq!(overview.passkeys.len(), 1);
    assert!(overview.linked_accounts.is_empty());

    let methods = fx.services.auth.methods();
    assert_eq!(methods.providers[0].id, "test");
    assert_eq!(
        methods.text_channels,
        [TextChannel::Sms, TextChannel::Whatsapp]
    );
}

#[derive(Clone, Copy)]
struct Forgery {
    kind: &'static str,
    origin: &'static str,
    cross_origin: bool,
    other_challenge: bool,
    rp_id: &'static str,
    flags: u8,
    sign_count: u32,
    bad_signature: bool,
}

impl Forgery {
    const GENUINE: Self = Self {
        kind: "webauthn.get",
        origin: ORIGIN,
        cross_origin: false,
        other_challenge: false,
        rp_id: "app.test",
        flags: 0x05,
        sign_count: 10,
        bad_signature: false,
    };

    fn assertion(self, challenge_id: uuid::Uuid, challenge: &[u8]) -> PasskeyAssertionRequest {
        let mut data = fake_digest(self.rp_id.as_bytes()).to_vec();
        data.push(self.flags);
        data.extend_from_slice(&self.sign_count.to_be_bytes());
        let challenge = if self.other_challenge {
            b64(b"a challenge captured earlier")
        } else {
            b64(challenge)
        };
        let client = format!(
            r#"{{"type":"{}","challenge":"{challenge}","origin":"{}","crossOrigin":{}}}"#,
            self.kind, self.origin, self.cross_origin
        )
        .into_bytes();
        let mut message = data.clone();
        message.extend_from_slice(&fake_digest(&client));
        let signer: &[u8] = if self.bad_signature {
            b"pk-mallory"
        } else {
            b"pk-alice"
        };
        PasskeyAssertionRequest {
            challenge_id,
            credential_id: b"cred-1".to_vec(),
            client_data_json: client,
            authenticator_data: data,
            signature: FakeCrypto::sign(signer, &message),
            user_handle: None,
        }
    }
}

fn sign_in_forgeries() -> [(&'static str, Forgery); 8] {
    let genuine = Forgery::GENUINE;
    [
        (
            "the registration type",
            Forgery {
                kind: "webauthn.create",
                ..genuine
            },
        ),
        (
            "a replayed challenge",
            Forgery {
                other_challenge: true,
                ..genuine
            },
        ),
        (
            "another origin",
            Forgery {
                origin: "https://evil.test",
                ..genuine
            },
        ),
        (
            "an iframe on another site",
            Forgery {
                cross_origin: true,
                ..genuine
            },
        ),
        (
            "another relying party",
            Forgery {
                rp_id: "evil.test",
                ..genuine
            },
        ),
        (
            "no user presence",
            Forgery {
                flags: 0x04,
                ..genuine
            },
        ),
        (
            "no user verification",
            Forgery {
                flags: 0x01,
                ..genuine
            },
        ),
        (
            "another key's signature",
            Forgery {
                bad_signature: true,
                ..genuine
            },
        ),
    ]
}

async fn sign_in_with(fx: &Fixture, forgery: Forgery) -> Result<(), AppError> {
    let options = fx.services.passkeys.login_options().await.unwrap();
    fx.services
        .passkeys
        .login(
            forgery.assertion(options.challenge_id, &options.public_key.challenge),
            ClientInfo::default(),
            None,
        )
        .await
        .map(|_| ())
}

#[tokio::test]
async fn passkey_sign_in_rejects_every_forged_field() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    register_passkey(&fx, &alice.actor, b"cred-1").await;

    for (what, forgery) in sign_in_forgeries() {
        let err = sign_in_with(&fx, forgery).await.unwrap_err();
        assert_eq!(err.code(), "invalid_passkey", "{what}");
    }
    // The genuine assertion works, which shows each forgery failed on its one field.
    sign_in_with(&fx, Forgery::GENUINE).await.unwrap();
    // A counter that goes backwards means a cloned authenticator.
    let cloned = Forgery {
        sign_count: 9,
        ..Forgery::GENUINE
    };
    let err = sign_in_with(&fx, cloned).await.unwrap_err();
    assert_eq!(err.code(), "invalid_passkey");
}

#[derive(Clone, Copy)]
struct Registration {
    kind: &'static str,
    origin: &'static str,
    cross_origin: bool,
    other_challenge: bool,
    rp_id: &'static str,
    flags: u8,
    client_data_len: Option<usize>,
}

impl Registration {
    const GENUINE: Self = Self {
        kind: "webauthn.create",
        origin: ORIGIN,
        cross_origin: false,
        other_challenge: false,
        rp_id: "app.test",
        flags: 0x45,
        client_data_len: None,
    };

    async fn register(
        self,
        fx: &Fixture,
        actor: &application::actor::Actor,
    ) -> Result<(), AppError> {
        let options = fx
            .services
            .passkeys
            .registration_options(actor)
            .await
            .unwrap();
        let challenge = if self.other_challenge {
            b64(b"old")
        } else {
            b64(&options.public_key.challenge)
        };
        let mut client = format!(
            r#"{{"type":"{}","challenge":"{challenge}","origin":"{}","crossOrigin":{}"#,
            self.kind, self.origin, self.cross_origin
        );
        if let Some(len) = self.client_data_len {
            let filler = len - client.len() - r#","padding":""}"#.len();
            client.push_str(r#","padding":""#);
            client.push_str(&"x".repeat(filler));
            client.push('"');
        }
        client.push('}');
        let mut data = fake_digest(self.rp_id.as_bytes()).to_vec();
        data.push(self.flags);
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(&[0; 16]);
        data.extend_from_slice(&6u16.to_be_bytes());
        data.extend_from_slice(b"cred-1");
        fx.services
            .passkeys
            .register(
                actor,
                RegisterPasskeyRequest {
                    challenge_id: options.challenge_id,
                    name: "Key".to_owned(),
                    credential_id: b"cred-1".to_vec(),
                    client_data_json: client.into_bytes(),
                    authenticator_data: data,
                    public_key: b"pk-alice".to_vec(),
                    public_key_algorithm: -7,
                    transports: Vec::new(),
                },
            )
            .await
            .map(|_| ())
    }
}

#[tokio::test]
async fn passkey_registration_rejects_every_forged_field() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    let genuine = Registration::GENUINE;

    for (what, forgery) in [
        (
            "the sign-in type",
            Registration {
                kind: "webauthn.get",
                ..genuine
            },
        ),
        (
            "a replayed challenge",
            Registration {
                other_challenge: true,
                ..genuine
            },
        ),
        (
            "another origin",
            Registration {
                origin: "https://evil.test",
                ..genuine
            },
        ),
        (
            "an iframe on another site",
            Registration {
                cross_origin: true,
                ..genuine
            },
        ),
        (
            "another relying party",
            Registration {
                rp_id: "evil.test",
                ..genuine
            },
        ),
        (
            "no user presence",
            Registration {
                flags: 0x44,
                ..genuine
            },
        ),
    ] {
        let err = forgery.register(&fx, &alice.actor).await.unwrap_err();
        assert_eq!(err.code(), "invalid_passkey", "{what}");
    }
    assert!(
        fx.services
            .passkeys
            .list(&alice.actor)
            .await
            .unwrap()
            .is_empty()
    );

    // The same shape, unforged, registers: each forgery failed on its one field.
    genuine.register(&fx, &alice.actor).await.unwrap();
}

#[tokio::test]
async fn ceremony_fields_may_be_16_kib_and_no_more() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    let sized = |len| Registration {
        client_data_len: Some(len),
        ..Registration::GENUINE
    };

    let err = sized(16 * 1024 + 1)
        .register(&fx, &alice.actor)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "invalid_passkey");
    sized(16 * 1024).register(&fx, &alice.actor).await.unwrap();
}

#[tokio::test]
async fn a_passkey_is_offered_for_stepping_up() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    let methods = fx
        .services
        .reauth
        .methods(&alice.actor)
        .await
        .unwrap()
        .methods;
    assert!(!methods.contains(&application::auth::dto::ReauthMethod::Passkey));

    register_passkey(&fx, &alice.actor, b"cred-1").await;
    let methods = fx
        .services
        .reauth
        .methods(&alice.actor)
        .await
        .unwrap()
        .methods;
    assert_eq!(methods[0], application::auth::dto::ReauthMethod::Passkey);
}

#[tokio::test]
async fn linking_checks_who_holds_the_provider_account() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    fx.providers
        .add_account("code-a", profile("sub-a", "alice@example.com", true));
    fx.providers
        .add_account("code-a2", profile("sub-a2", "alice@example.com", true));

    for _ in 0..2 {
        let done = oauth_callback(&fx, "code-a", Some(&alice.actor))
            .await
            .unwrap();
        assert!(matches!(done.outcome, OAuthOutcome::Linked));
    }
    assert_eq!(
        fx.services
            .oauth
            .identities(&alice.actor)
            .await
            .unwrap()
            .len(),
        1
    );

    let err = oauth_callback(&fx, "code-a2", Some(&alice.actor))
        .await
        .unwrap_err();
    assert_eq!(err.code(), "provider_linked");
    assert_eq!(
        fx.services
            .oauth
            .identities(&alice.actor)
            .await
            .unwrap()
            .len(),
        1
    );
}

async fn confirm_change(fx: &Fixture, new: &str) -> Result<(), AppError> {
    fx.services
        .auth
        .confirm_email_change(
            ConfirmEmailRequest {
                token: SecretInput(fx.mail.token_for(new)),
            },
            None,
        )
        .await
}

#[tokio::test]
async fn an_email_change_loses_to_whoever_took_the_address_meanwhile() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    fx.services
        .account
        .request_email_change(
            &alice.actor,
            ChangeEmailRequest {
                email: "new@example.com".to_owned(),
            },
        )
        .await
        .unwrap();
    let link = fx.mail.token_for("new@example.com");

    fx.register("new@example.com").await;

    let err = fx
        .services
        .auth
        .confirm_email_change(
            ConfirmEmailRequest {
                token: SecretInput(link),
            },
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "email_taken");
    let user = fx
        .db
        .with(|state| state.users[&alice.actor.user_id].clone());
    assert_eq!(user.email().as_str(), "alice@example.com");
}

#[tokio::test]
async fn undoing_an_email_change_fails_once_the_old_address_is_taken() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    fx.services
        .account
        .request_email_change(
            &alice.actor,
            ChangeEmailRequest {
                email: "new@example.com".to_owned(),
            },
        )
        .await
        .unwrap();
    let cancel = fx.mail.token_for("alice@example.com");
    confirm_change(&fx, "new@example.com").await.unwrap();

    let mut someone = register_request("alice@example.com");
    someone.username = "someone".to_owned();
    fx.services
        .auth
        .register(someone, ClientInfo::default())
        .await
        .unwrap();

    let err = fx
        .services
        .auth
        .cancel_email_change(application::auth::dto::CancelEmailChangeRequest {
            token: SecretInput(cancel),
        })
        .await
        .unwrap_err();
    assert_eq!(err.code(), "email_taken");
    let user = fx
        .db
        .with(|state| state.users[&alice.actor.user_id].clone());
    assert_eq!(user.email().as_str(), "new@example.com");
}

#[tokio::test]
async fn texts_only_go_to_the_allowed_countries() {
    let fx = Fixture::with(|settings| {
        settings.text_countries = vec![domain::user::CallingCode::parse("+41").unwrap()];
    });
    let alice = fx.user("alice@example.com").await;
    let add = |phone: &str| AddPhoneRequest {
        phone: phone.to_owned(),
        channel: TextChannel::Sms,
    };

    let err = fx
        .services
        .account
        .add_phone(&alice.actor, add("+49 170 1234567"))
        .await
        .unwrap_err();
    let AppError::Validation(errors) = err else {
        panic!("expected a field error");
    };
    assert_eq!(errors.fields()[0].code, "phone_country_unsupported");
    assert!(fx.texts.sent().is_empty());

    fx.services
        .account
        .add_phone(&alice.actor, add("+41 79 123 45 67"))
        .await
        .unwrap();
    assert_eq!(fx.texts.sent().len(), 1);
}
