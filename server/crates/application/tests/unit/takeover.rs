use application::{
    account::dto::{AddPhoneRequest, ChangeEmailRequest},
    auth::{
        Browser, Registered,
        dto::{
            ConfirmEmailRequest, ForgotPasswordRequest, LoginRequest, ResetPasswordRequest,
            VerifyEmailRequest,
        },
    },
    passwordless::dto::TextChannel,
};
use domain::{
    clock::Clock,
    database::Database,
    identity::{IdentityRepository, NewIdentity},
    mfa::{MfaChallenge, MfaRepository},
    passkey::{NewPasskey, PasskeyName, PasskeyRepository, PublicKeyAlgorithm},
    secret::TokenHash,
    session::ClientInfo,
    user::{PhoneNumber, UserId, UserRepository},
    user_token::TokenPurpose,
};

use crate::support::{Fixture, PASSWORD, code_in, register_request, secret};

async fn attach_everything(fx: &Fixture, user: UserId) {
    let mut conn = fx.db.connection().await.unwrap();
    conn.create_identity(&NewIdentity {
        id: domain::identity::IdentityId::generate(),
        user_id: user,
        provider: "test".to_owned(),
        subject: "attacker".to_owned(),
        email: None,
    })
    .await
    .unwrap();
    conn.create_passkey(&NewPasskey {
        id: domain::passkey::PasskeyId::generate(),
        user_id: user,
        credential_id: b"attacker-key".to_vec(),
        public_key: vec![1, 2, 3],
        algorithm: PublicKeyAlgorithm::Es256,
        sign_count: 0,
        transports: Vec::new(),
        name: PasskeyName::parse("Attacker").unwrap(),
    })
    .await
    .unwrap();
    conn.start_totp_setup(user, b"sealed", fx.clock.now())
        .await
        .unwrap();
    conn.confirm_totp(user, fx.clock.now(), 1).await.unwrap();
    conn.replace_recovery_codes(user, &[TokenHash::new([9; 32])])
        .await
        .unwrap();
    let phone = PhoneNumber::parse("+15550001111").unwrap();
    conn.set_user_phone(user, Some((&phone, fx.clock.now())))
        .await
        .unwrap();
    conn.create_mfa_challenge(&MfaChallenge::start(
        user,
        TokenHash::new([8; 32]),
        fx.clock.now(),
    ))
    .await
    .unwrap();
}

fn assert_nothing_attached(fx: &Fixture, user: UserId) {
    fx.db.with(|state| {
        assert!(state.identities.iter().all(|i| i.user_id != user));
        assert!(state.passkeys.iter().all(|p| p.user_id != user));
        assert!(!state.totp.contains_key(&user));
        assert!(state.recovery_codes.iter().all(|(owner, _)| *owner != user));
        assert!(state.users[&user].phone().is_none());
        assert!(state.mfa_challenges.iter().all(|c| c.user_id != user));
        assert!(state.sessions.values().all(|s| s.user_id() != user));
    });
}

async fn password_works(fx: &Fixture, email: &str, password: &str) -> bool {
    fx.services
        .auth
        .login(
            LoginRequest {
                identifier: email.to_owned(),
                password: secret(password),
            },
            ClientInfo::default(),
            None,
        )
        .await
        .is_ok()
}

async fn verify(fx: &Fixture, email: &str, browser: Browser<'_>) {
    fx.services
        .auth
        .verify_email(
            VerifyEmailRequest {
                token: application::dto::SecretInput(fx.mail.token_for(email)),
            },
            browser,
        )
        .await
        .unwrap();
}

async fn registration_mark(fx: &Fixture, email: &str) -> domain::secret::Secret {
    match fx
        .services
        .auth
        .register(register_request(email), ClientInfo::default())
        .await
        .unwrap()
    {
        Registered::VerificationPending { browser, .. } => browser,
        Registered::SignedIn(_) => panic!("expected verification to be pending"),
    }
}

async fn sign_in_with_emailed_code(fx: &Fixture, email: &str) -> application::auth::LoginOutcome {
    fx.services
        .passwordless
        .request_email_code(application::passwordless::dto::EmailCodeRequest {
            email: email.to_owned(),
        })
        .await
        .unwrap();
    let mail = fx.mail.last_to(email).unwrap();
    let code = code_in(&mail.body);
    fx.services
        .passwordless
        .verify_email_code(
            application::passwordless::dto::VerifyEmailCodeRequest {
                email: email.to_owned(),
                code: secret(&code),
            },
            ClientInfo::default(),
            None,
        )
        .await
        .unwrap()
}

async fn reset_password(fx: &Fixture, email: &str) {
    fx.services
        .auth
        .request_password_reset(ForgotPasswordRequest {
            email: email.to_owned(),
        })
        .await
        .unwrap();
    fx.services
        .auth
        .reset_password(ResetPasswordRequest {
            token: application::dto::SecretInput(fx.mail.token_for(email)),
            password: secret("the owner's new password"),
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn an_unverified_account_cannot_add_a_way_in() {
    let fx = Fixture::new();
    let attacker = fx.unverified_user("victim@example.com").await;
    let actor = &attacker.actor;

    let errors = [
        fx.services.mfa.start_totp(actor).await.map(|_| ()),
        fx.services
            .passkeys
            .registration_options(actor)
            .await
            .map(|_| ()),
        fx.services
            .account
            .add_phone(
                actor,
                AddPhoneRequest {
                    phone: "+15550001111".to_owned(),
                    channel: TextChannel::Sms,
                },
            )
            .await,
        fx.services
            .oauth
            .start("test", Some(actor), None)
            .await
            .map(|_| ()),
    ];
    for result in errors {
        assert_eq!(result.unwrap_err().code(), "email_unverified");
    }
}

#[tokio::test]
async fn the_first_proof_of_the_address_evicts_earlier_claimants() {
    let fx = Fixture::new();
    let attacker = fx.unverified_user("victim@example.com").await;
    let victim_id = attacker.actor.user_id;
    attach_everything(&fx, victim_id).await;

    reset_password(&fx, "victim@example.com").await;

    assert_nothing_attached(&fx, victim_id);
    assert!(
        fx.db
            .with(|state| !state.sessions.contains_key(&attacker.session.id()))
    );
}

#[tokio::test]
async fn later_proofs_keep_the_owners_sign_in_methods() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    attach_everything(&fx, alice.actor.user_id).await;

    reset_password(&fx, "alice@example.com").await;

    fx.db.with(|state| {
        let user = alice.actor.user_id;
        assert_eq!(
            state
                .identities
                .iter()
                .filter(|i| i.user_id == user)
                .count(),
            1
        );
        assert_eq!(
            state.passkeys.iter().filter(|p| p.user_id == user).count(),
            1
        );
        assert!(state.totp.contains_key(&user));
        assert!(state.users[&user].phone().is_some());
        assert!(state.sessions.values().all(|s| s.user_id() != user));
        assert!(state.mfa_challenges.iter().all(|c| c.user_id != user));
    });
}

#[tokio::test]
async fn an_emailed_code_is_a_first_proof_too() {
    let fx = Fixture::new();
    let attacker = fx.unverified_user("victim@example.com").await;
    attach_everything(&fx, attacker.actor.user_id).await;

    let signed_in = sign_in_with_emailed_code(&fx, "victim@example.com").await;

    let application::auth::LoginOutcome::SignedIn(signed_in) = signed_in else {
        panic!("the evicted authenticator app must not ask for a second step");
    };
    assert!(signed_in.me.user.email_verified);
    assert!(
        fx.db
            .with(|state| !state.sessions.contains_key(&attacker.session.id()))
    );
    fx.db.with(|state| {
        let user = attacker.actor.user_id;
        assert!(state.passkeys.iter().all(|p| p.user_id != user));
        assert!(!state.totp.contains_key(&user));
        assert_eq!(
            state
                .sessions
                .values()
                .filter(|s| s.user_id() == user)
                .count(),
            1
        );
    });
    assert!(!password_works(&fx, "victim@example.com", PASSWORD).await);
    let offer = fx.mail.last_to("victim@example.com").unwrap();
    assert_eq!(offer.template, "choose_password");
}

#[tokio::test]
async fn the_attackers_password_does_not_survive_the_verification_link() {
    let fx = Fixture::new();
    fx.unverified_user("victim@example.com").await;

    verify(&fx, "victim@example.com", Browser::default()).await;

    assert!(!password_works(&fx, "victim@example.com", PASSWORD).await);
    let offer = fx.mail.last_to("victim@example.com").unwrap();
    assert_eq!(offer.template, "choose_password");
    fx.services
        .auth
        .reset_password(ResetPasswordRequest {
            token: application::dto::SecretInput(fx.mail.token_for("victim@example.com")),
            password: secret("the owner's own password"),
        })
        .await
        .unwrap();
    assert!(password_works(&fx, "victim@example.com", "the owner's own password").await);
}

#[tokio::test]
async fn with_required_verification_the_attackers_password_does_not_survive_either() {
    let fx = Fixture::with(|settings| settings.require_email_verification = true);
    registration_mark(&fx, "victim@example.com").await;

    verify(&fx, "victim@example.com", Browser::default()).await;

    assert!(!password_works(&fx, "victim@example.com", PASSWORD).await);
}

#[tokio::test]
async fn the_registering_browser_keeps_its_password() {
    let fx = Fixture::with(|settings| settings.require_email_verification = true);
    let browser = registration_mark(&fx, "alice@example.com").await;

    verify(
        &fx,
        "alice@example.com",
        Browser {
            registration: Some(&browser),
            ..Browser::default()
        },
    )
    .await;

    assert!(password_works(&fx, "alice@example.com", PASSWORD).await);
    assert!(
        fx.mail
            .sent()
            .iter()
            .all(|mail| mail.template != "choose_password")
    );
}

#[tokio::test]
async fn the_registering_session_keeps_its_password() {
    let fx = Fixture::new();
    let alice = fx.register("alice@example.com").await;

    verify(
        &fx,
        "alice@example.com",
        Browser {
            session: Some(&alice.token),
            ..Browser::default()
        },
    )
    .await;

    assert!(password_works(&fx, "alice@example.com", PASSWORD).await);
}

#[tokio::test]
async fn a_mark_for_another_account_or_a_forged_one_keeps_nothing() {
    let fx = Fixture::with(|settings| settings.require_email_verification = true);
    let attackers_mark = registration_mark(&fx, "attacker@evil.test").await;
    registration_mark(&fx, "victim@example.com").await;
    verify(
        &fx,
        "victim@example.com",
        Browser {
            registration: Some(&attackers_mark),
            ..Browser::default()
        },
    )
    .await;
    assert!(!password_works(&fx, "victim@example.com", PASSWORD).await);

    let bobs_mark = registration_mark(&fx, "bob@example.com").await;
    let (_, digest) = bobs_mark.expose().split_once('.').unwrap();
    let forged = domain::secret::Secret::new(format!("99999999999.{digest}"));
    verify(
        &fx,
        "bob@example.com",
        Browser {
            registration: Some(&forged),
            ..Browser::default()
        },
    )
    .await;
    assert!(!password_works(&fx, "bob@example.com", PASSWORD).await);
}

#[tokio::test]
async fn a_reset_cancels_a_pending_email_change() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    fx.services
        .account
        .request_email_change(
            &alice.actor,
            ChangeEmailRequest {
                email: "attacker@evil.test".to_owned(),
            },
        )
        .await
        .unwrap();
    let change = fx.mail.token_for("attacker@evil.test");

    reset_password(&fx, "alice@example.com").await;

    let err = fx
        .services
        .auth
        .confirm_email_change(
            ConfirmEmailRequest {
                token: application::dto::SecretInput(change),
            },
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "invalid_token");
    let user = fx
        .db
        .with(|state| state.users[&alice.actor.user_id].clone());
    assert_eq!(user.email().as_str(), "alice@example.com");
}

#[tokio::test]
async fn signing_out_everywhere_cancels_pending_links_and_second_steps() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    fx.services
        .account
        .request_email_change(
            &alice.actor,
            ChangeEmailRequest {
                email: "attacker@evil.test".to_owned(),
            },
        )
        .await
        .unwrap();
    let change = fx.mail.token_for("attacker@evil.test");
    let mut conn = fx.db.connection().await.unwrap();
    conn.create_mfa_challenge(&MfaChallenge::start(
        alice.actor.user_id,
        TokenHash::new([8; 32]),
        fx.clock.now(),
    ))
    .await
    .unwrap();
    drop(conn);

    fx.services
        .auth
        .logout_everywhere(&alice.actor)
        .await
        .unwrap();

    let err = fx
        .services
        .auth
        .confirm_email_change(
            ConfirmEmailRequest {
                token: application::dto::SecretInput(change),
            },
            None,
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "invalid_token");
    fx.db.with(|state| {
        assert!(state.mfa_challenges.is_empty());
        assert!(state.tokens.iter().all(|token| matches!(
            token.purpose,
            TokenPurpose::EmailVerification | TokenPurpose::EmailChangeCancel
        )));
    });
}

#[tokio::test]
async fn disabling_an_account_cancels_its_pending_links() {
    let fx = Fixture::new();
    let admin = fx.admin("admin@example.com").await;
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

    fx.services
        .admin
        .set_status(
            &admin.actor,
            alice.actor.user_id,
            application::admin::AccountStatus::Disabled,
        )
        .await
        .unwrap();

    fx.db.with(|state| {
        assert!(
            state
                .tokens
                .iter()
                .filter(|token| token.user_id == alice.actor.user_id)
                .all(|token| matches!(
                    token.purpose,
                    TokenPurpose::EmailVerification | TokenPurpose::EmailChangeCancel
                ))
        );
    });
}

#[tokio::test]
async fn a_taken_address_gets_a_decoy_mark_that_looks_real() {
    let fx = Fixture::with(|settings| settings.require_email_verification = true);
    let real = registration_mark(&fx, "alice@example.com").await;
    let mut again = register_request("alice@example.com");
    again.username = "someone".to_owned();
    let Registered::VerificationPending { browser: decoy, .. } = fx
        .services
        .auth
        .register(again, ClientInfo::default())
        .await
        .unwrap()
    else {
        panic!("expected verification to be pending");
    };

    let (real_expiry, real_digest) = real.expose().split_once('.').unwrap();
    let (decoy_expiry, decoy_digest) = decoy.expose().split_once('.').unwrap();
    assert_eq!(decoy_expiry, real_expiry);
    assert_eq!(decoy_digest.len(), real_digest.len());
    assert_ne!(decoy_digest, real_digest);
}

#[tokio::test]
async fn known_device_marks_are_per_account_and_expire() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await.actor.user_id;
    let bob = fx.user("bob@example.com").await.actor.user_id;
    let mark = fx.services.auth.known_device_mark(alice);

    assert!(fx.services.auth.is_known_device(alice, &mark));
    assert!(!fx.services.auth.is_known_device(bob, &mark));
    assert!(
        !fx.services
            .auth
            .is_known_device(alice, &domain::secret::Secret::new("0.forged"))
    );
    fx.clock
        .advance(application::auth::KNOWN_DEVICE_TTL - time::Duration::days(1));
    assert!(fx.services.auth.is_known_device(alice, &mark));
    fx.clock.advance(time::Duration::days(1));
    assert!(!fx.services.auth.is_known_device(alice, &mark));
}
