use domain::{
    error::StorageError,
    identity::{
        IDENTITY_PROVIDER_UNIQUE_CONSTRAINT, IDENTITY_SUBJECT_UNIQUE_CONSTRAINT,
        IdentityRepository, NewIdentity, OAuthFlow,
    },
    mfa::{MAX_MFA_ATTEMPTS, MfaChallenge, MfaRepository},
    one_time_code::{
        CodeChannel, CodePurpose, MAX_CODE_ATTEMPTS, OneTimeCode, OneTimeCodeRepository,
    },
    passkey::{
        CHALLENGE_TTL, ChallengeId, ChallengePurpose, NewPasskey,
        PASSKEY_CREDENTIAL_UNIQUE_CONSTRAINT, PasskeyName, PasskeyRepository, PublicKeyAlgorithm,
        WebAuthnChallenge,
    },
    secret::{Secret, TokenHash},
    user::{
        EMAIL_UNIQUE_CONSTRAINT, Email, NewUser, PHONE_UNIQUE_CONSTRAINT, PhoneNumber,
        USERNAME_UNIQUE_CONSTRAINT, UserRepository, Username,
    },
};
use sqlx::PgPool;
use time::{Duration, OffsetDateTime};

use crate::support::{conn, new_user, user};

fn now() -> OffsetDateTime {
    domain::clock::truncate_to_micros(OffsetDateTime::now_utc())
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn usernames_phones_and_emails_are_unique(pool: PgPool) {
    let mut conn = conn(&pool).await;
    let alice = conn
        .create_user(&NewUser {
            id: domain::user::UserId::generate(),
            password_hash: None,
            email_verified_at: Some(now()),
            ..new_user("alice@example.com")
        })
        .await
        .unwrap();
    assert!(!alice.has_password());
    assert!(alice.is_email_verified());
    let bob = user(&mut conn, "bob@example.com").await;

    let found = conn
        .find_user_by_username(&Username::parse("ALICE").unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.id(), alice.id());

    let err = conn
        .set_user_username(bob.id(), &Username::parse("alice").unwrap())
        .await
        .unwrap_err();
    assert!(err.is_unique_violation(USERNAME_UNIQUE_CONSTRAINT));
    let renamed = conn
        .set_user_username(alice.id(), &Username::parse("alice.l").unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(renamed.username().as_str(), "alice.l");

    let phone = PhoneNumber::parse("+491701234567").unwrap();
    let at = now();
    let with_phone = conn
        .set_user_phone(alice.id(), Some((&phone, at)))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(with_phone.verified_phone(), Some(&phone));
    assert_eq!(
        conn.find_user_by_phone(&phone).await.unwrap().unwrap().id(),
        alice.id()
    );
    let err = conn
        .set_user_phone(bob.id(), Some((&phone, at)))
        .await
        .unwrap_err();
    assert!(err.is_unique_violation(PHONE_UNIQUE_CONSTRAINT));

    let moved = conn
        .change_user_email(alice.id(), &Email::parse("alice@new.example").unwrap(), at)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(moved.email().as_str(), "alice@new.example");
    assert_eq!(moved.email_verified_at(), Some(at));
    let err = conn
        .change_user_email(bob.id(), &Email::parse("alice@new.example").unwrap(), at)
        .await
        .unwrap_err();
    assert!(err.is_unique_violation(EMAIL_UNIQUE_CONSTRAINT));

    let removed = conn
        .set_user_phone(alice.id(), None)
        .await
        .unwrap()
        .unwrap();
    assert!(removed.phone().is_none());
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn one_time_codes(pool: PgPool) {
    let mut conn = conn(&pool).await;
    let alice = user(&mut conn, "alice@example.com").await;
    let phone = PhoneNumber::parse("+15551234567").unwrap();
    let code = OneTimeCode {
        target: Some(phone.clone()),
        ..OneTimeCode::issue(
            alice.id(),
            CodePurpose::PhoneVerification,
            CodeChannel::Whatsapp,
            TokenHash::new([1; 32]),
            now(),
        )
    };
    conn.replace_one_time_code(&code).await.unwrap();
    assert_eq!(
        conn.reserve_code_attempt(alice.id(), CodePurpose::PhoneVerification, now())
            .await
            .unwrap()
            .unwrap()
            .attempts,
        1
    );
    let stored = conn
        .find_one_time_code(alice.id(), CodePurpose::PhoneVerification)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.target, Some(phone));
    assert_eq!(stored.channel, CodeChannel::Whatsapp);
    assert_eq!(stored.attempts, 1);

    conn.replace_one_time_code(&code).await.unwrap();
    assert_eq!(
        conn.find_one_time_code(alice.id(), CodePurpose::PhoneVerification)
            .await
            .unwrap()
            .unwrap()
            .attempts,
        0
    );
    assert!(
        conn.find_one_time_code(alice.id(), CodePurpose::Login)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        conn.delete_expired_one_time_codes(now() + Duration::hours(1))
            .await
            .unwrap(),
        1
    );
    assert!(
        !conn
            .delete_one_time_code(alice.id(), CodePurpose::PhoneVerification)
            .await
            .unwrap()
    );
    assert!(
        conn.reserve_code_attempt(alice.id(), CodePurpose::Login, now())
            .await
            .unwrap()
            .is_none()
    );
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn code_attempts_stop_at_the_limit_and_at_expiry(pool: PgPool) {
    let mut conn = conn(&pool).await;
    let alice = user(&mut conn, "alice@example.com").await;
    let code = OneTimeCode::issue(
        alice.id(),
        CodePurpose::Login,
        CodeChannel::Email,
        TokenHash::new([1; 32]),
        now(),
    );
    conn.replace_one_time_code(&code).await.unwrap();

    assert!(
        conn.reserve_code_attempt(alice.id(), CodePurpose::Login, code.expires_at)
            .await
            .unwrap()
            .is_none(),
        "an expired code takes no guesses"
    );
    for attempt in 1..=MAX_CODE_ATTEMPTS {
        let reserved = conn
            .reserve_code_attempt(alice.id(), CodePurpose::Login, now())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(reserved.attempts, attempt);
    }
    assert!(
        conn.reserve_code_attempt(alice.id(), CodePurpose::Login, now())
            .await
            .unwrap()
            .is_none()
    );
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn concurrent_attempts_cannot_exceed_the_limits(pool: PgPool) {
    let mut conn = conn(&pool).await;
    let alice = user(&mut conn, "alice@example.com").await;
    conn.replace_one_time_code(&OneTimeCode::issue(
        alice.id(),
        CodePurpose::Login,
        CodeChannel::Email,
        TokenHash::new([1; 32]),
        now(),
    ))
    .await
    .unwrap();
    let challenge = MfaChallenge::start(alice.id(), TokenHash::new([5; 32]), now());
    conn.create_mfa_challenge(&challenge).await.unwrap();
    drop(conn);

    let tasks: Vec<_> = (0..20)
        .map(|_| {
            let pool = pool.clone();
            let token_hash = challenge.token_hash;
            let user_id = alice.id();
            tokio::spawn(async move {
                let mut conn = crate::support::conn(&pool).await;
                let code = conn
                    .reserve_code_attempt(user_id, CodePurpose::Login, now())
                    .await
                    .unwrap()
                    .is_some();
                let mfa = conn
                    .reserve_mfa_attempt(&token_hash, now())
                    .await
                    .unwrap()
                    .is_some();
                (code, mfa)
            })
        })
        .collect();
    let mut codes = 0;
    let mut mfa = 0;
    for task in tasks {
        let (code, challenge) = task.await.unwrap();
        codes += u32::from(code);
        mfa += u32::from(challenge);
    }
    assert_eq!(codes, MAX_CODE_ATTEMPTS);
    assert_eq!(mfa, MAX_MFA_ATTEMPTS);
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn identities_and_oauth_flows(pool: PgPool) {
    let mut conn = conn(&pool).await;
    let alice = user(&mut conn, "alice@example.com").await;
    let bob = user(&mut conn, "bob@example.com").await;
    let identity = NewIdentity {
        id: domain::identity::IdentityId::generate(),
        user_id: alice.id(),
        provider: "github".to_owned(),
        subject: "42".to_owned(),
        email: Some("alice@example.com".to_owned()),
    };
    let created = conn.create_identity(&identity).await.unwrap();
    assert_eq!(
        conn.find_identity("github", "42")
            .await
            .unwrap()
            .unwrap()
            .id,
        created.id
    );

    let err: StorageError = conn
        .create_identity(&NewIdentity {
            id: domain::identity::IdentityId::generate(),
            user_id: bob.id(),
            ..identity.clone()
        })
        .await
        .unwrap_err();
    assert!(err.is_unique_violation(IDENTITY_SUBJECT_UNIQUE_CONSTRAINT));
    let err = conn
        .create_identity(&NewIdentity {
            id: domain::identity::IdentityId::generate(),
            subject: "43".to_owned(),
            ..identity.clone()
        })
        .await
        .unwrap_err();
    assert!(err.is_unique_violation(IDENTITY_PROVIDER_UNIQUE_CONSTRAINT));

    conn.touch_identity(created.id, now()).await.unwrap();
    assert_eq!(
        conn.list_user_identities(alice.id()).await.unwrap().len(),
        1
    );
    assert!(
        conn.delete_user_identity(alice.id(), "github")
            .await
            .unwrap()
    );
    assert!(
        !conn
            .delete_user_identity(alice.id(), "github")
            .await
            .unwrap()
    );

    let flow = OAuthFlow {
        state_hash: TokenHash::new([9; 32]),
        provider: "github".to_owned(),
        pkce_verifier: Secret::new("verifier"),
        nonce: "nonce".to_owned(),
        link_user: Some(alice.id()),
        redirect_to: "/settings/security".to_owned(),
        expires_at: now() + Duration::minutes(10),
    };
    conn.create_oauth_flow(&flow).await.unwrap();
    let consumed = conn
        .consume_oauth_flow(&flow.state_hash, now())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(consumed.pkce_verifier.expose(), "verifier");
    assert_eq!(consumed.link_user, Some(alice.id()));
    assert!(
        conn.consume_oauth_flow(&flow.state_hash, now())
            .await
            .unwrap()
            .is_none()
    );
    conn.create_oauth_flow(&flow).await.unwrap();
    assert_eq!(
        conn.delete_expired_oauth_flows(now() + Duration::hours(1))
            .await
            .unwrap(),
        1
    );
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn passkeys_and_challenges(pool: PgPool) {
    let mut conn = conn(&pool).await;
    let alice = user(&mut conn, "alice@example.com").await;
    let bob = user(&mut conn, "bob@example.com").await;
    let new = NewPasskey {
        id: domain::passkey::PasskeyId::generate(),
        user_id: alice.id(),
        credential_id: vec![1, 2, 3],
        public_key: vec![4, 5, 6],
        algorithm: PublicKeyAlgorithm::Rs256,
        sign_count: 7,
        transports: vec!["usb".to_owned(), "nfc".to_owned()],
        name: PasskeyName::parse("YubiKey").unwrap(),
    };
    let passkey = conn.create_passkey(&new).await.unwrap();
    assert_eq!(passkey.algorithm, PublicKeyAlgorithm::Rs256);
    assert_eq!(passkey.transports, ["usb", "nfc"]);
    let again = NewPasskey {
        id: domain::passkey::PasskeyId::generate(),
        ..new.clone()
    };
    let err = conn.create_passkey(&again).await.unwrap_err();
    assert!(err.is_unique_violation(PASSKEY_CREDENTIAL_UNIQUE_CONSTRAINT));

    conn.record_passkey_use(passkey.id, 8, now()).await.unwrap();
    let found = conn
        .find_passkey_by_credential(&[1, 2, 3])
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found.sign_count, 8);
    assert!(found.last_used_at.is_some());

    let name = PasskeyName::parse("Backup key").unwrap();
    assert!(
        conn.rename_user_passkey(bob.id(), passkey.id, &name)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        conn.rename_user_passkey(alice.id(), passkey.id, &name)
            .await
            .unwrap()
            .unwrap()
            .name,
        name
    );
    assert_eq!(conn.count_user_passkeys(alice.id()).await.unwrap(), 1);
    assert_eq!(conn.list_user_passkeys(alice.id()).await.unwrap().len(), 1);
    assert!(
        !conn
            .delete_user_passkey(bob.id(), passkey.id)
            .await
            .unwrap()
    );
    assert!(
        conn.delete_user_passkey(alice.id(), passkey.id)
            .await
            .unwrap()
    );

    let challenge = WebAuthnChallenge {
        id: ChallengeId::generate(),
        challenge: vec![0; 32],
        purpose: ChallengePurpose::SecondFactor,
        user_id: Some(alice.id()),
        expires_at: now() + CHALLENGE_TTL,
    };
    conn.create_webauthn_challenge(&challenge).await.unwrap();
    assert!(
        conn.consume_webauthn_challenge(challenge.id, ChallengePurpose::Authentication, now())
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        conn.consume_webauthn_challenge(challenge.id, ChallengePurpose::SecondFactor, now())
            .await
            .unwrap(),
        Some(challenge.clone())
    );
    conn.create_webauthn_challenge(&challenge).await.unwrap();
    assert_eq!(
        conn.delete_expired_webauthn_challenges(now() + Duration::hours(1))
            .await
            .unwrap(),
        1
    );
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn authenticator_apps_recovery_codes_and_challenges(pool: PgPool) {
    let mut conn = conn(&pool).await;
    let alice = user(&mut conn, "alice@example.com").await;

    assert!(
        conn.start_totp_setup(alice.id(), b"sealed-1", now())
            .await
            .unwrap()
    );
    assert!(
        conn.start_totp_setup(alice.id(), b"sealed-2", now())
            .await
            .unwrap()
    );
    assert!(!conn.use_totp_step(alice.id(), 5).await.unwrap());
    assert!(conn.confirm_totp(alice.id(), now(), 10).await.unwrap());
    assert!(!conn.confirm_totp(alice.id(), now(), 11).await.unwrap());
    assert!(
        !conn
            .start_totp_setup(alice.id(), b"sealed-3", now())
            .await
            .unwrap()
    );
    let totp = conn.find_totp(alice.id()).await.unwrap().unwrap();
    assert_eq!(totp.sealed_secret, b"sealed-2");
    assert_eq!(totp.last_used_step, Some(10));

    assert!(!conn.use_totp_step(alice.id(), 10).await.unwrap());
    assert!(conn.use_totp_step(alice.id(), 11).await.unwrap());
    assert!(conn.delete_totp(alice.id()).await.unwrap());
    assert!(conn.find_totp(alice.id()).await.unwrap().is_none());

    let bob = user(&mut conn, "bob@example.com").await;
    let day = time::Duration::days(1);
    conn.start_totp_setup(alice.id(), b"abandoned", now() - day * 2)
        .await
        .unwrap();
    conn.start_totp_setup(bob.id(), b"confirmed", now() - day * 2)
        .await
        .unwrap();
    conn.confirm_totp(bob.id(), now() - day * 2, 1)
        .await
        .unwrap();
    assert_eq!(conn.delete_stale_totp_setups(now() - day).await.unwrap(), 1);
    assert!(conn.find_totp(alice.id()).await.unwrap().is_none());
    assert!(conn.find_totp(bob.id()).await.unwrap().is_some());

    let codes = [TokenHash::new([1; 32]), TokenHash::new([2; 32])];
    conn.replace_recovery_codes(alice.id(), &codes)
        .await
        .unwrap();
    assert_eq!(conn.count_recovery_codes(alice.id()).await.unwrap(), 2);
    assert!(
        conn.consume_recovery_code(alice.id(), &codes[0])
            .await
            .unwrap()
    );
    assert!(
        !conn
            .consume_recovery_code(alice.id(), &codes[0])
            .await
            .unwrap()
    );
    conn.replace_recovery_codes(alice.id(), &[]).await.unwrap();
    assert_eq!(conn.count_recovery_codes(alice.id()).await.unwrap(), 0);

    let challenge = MfaChallenge::start(alice.id(), TokenHash::new([5; 32]), now());
    conn.create_mfa_challenge(&challenge).await.unwrap();
    assert_eq!(
        conn.reserve_mfa_attempt(&challenge.token_hash, now())
            .await
            .unwrap()
            .unwrap()
            .attempts,
        1
    );
    assert!(
        conn.reserve_mfa_attempt(&challenge.token_hash, challenge.expires_at)
            .await
            .unwrap()
            .is_none(),
        "an expired challenge takes no answers"
    );
    assert_eq!(
        conn.find_mfa_challenge(&challenge.token_hash)
            .await
            .unwrap()
            .unwrap()
            .attempts,
        1
    );
    assert!(
        conn.delete_mfa_challenge(&challenge.token_hash)
            .await
            .unwrap()
    );
    assert!(
        !conn
            .delete_mfa_challenge(&challenge.token_hash)
            .await
            .unwrap()
    );
    conn.create_mfa_challenge(&challenge).await.unwrap();
    assert_eq!(
        conn.delete_expired_mfa_challenges(now() + Duration::hours(1))
            .await
            .unwrap(),
        1
    );
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn everything_attached_to_a_user_can_be_deleted_at_once(pool: PgPool) {
    let mut conn = conn(&pool).await;
    let alice = user(&mut conn, "alice@example.com").await;
    let bob = user(&mut conn, "bob@example.com").await;
    for (owner, n) in [(&alice, 1u8), (&alice, 2), (&bob, 3)] {
        conn.create_identity(&NewIdentity {
            id: domain::identity::IdentityId::generate(),
            user_id: owner.id(),
            provider: format!("provider-{n}"),
            subject: format!("subject-{n}"),
            email: None,
        })
        .await
        .unwrap();
        conn.create_passkey(&NewPasskey {
            id: domain::passkey::PasskeyId::generate(),
            user_id: owner.id(),
            credential_id: vec![n; 16],
            public_key: vec![n; 32],
            algorithm: PublicKeyAlgorithm::Es256,
            sign_count: 0,
            transports: Vec::new(),
            name: PasskeyName::parse("Key").unwrap(),
        })
        .await
        .unwrap();
        conn.create_mfa_challenge(&MfaChallenge::start(
            owner.id(),
            TokenHash::new([n; 32]),
            now(),
        ))
        .await
        .unwrap();
    }

    assert_eq!(conn.delete_user_identities(alice.id()).await.unwrap(), 2);
    assert_eq!(conn.delete_user_passkeys(alice.id()).await.unwrap(), 2);
    assert_eq!(
        conn.delete_user_mfa_challenges(alice.id()).await.unwrap(),
        2
    );
    assert_eq!(conn.delete_user_identities(alice.id()).await.unwrap(), 0);

    assert_eq!(conn.list_user_identities(bob.id()).await.unwrap().len(), 1);
    assert_eq!(conn.count_user_passkeys(bob.id()).await.unwrap(), 1);
    assert!(
        conn.reserve_mfa_attempt(&TokenHash::new([3; 32]), now())
            .await
            .unwrap()
            .is_some()
    );
}
