use axum::http::StatusCode;
use proto::v1;
use ring::{
    digest,
    signature::{Ed25519KeyPair, KeyPair},
};
use sqlx::PgPool;
use time::Duration;

use crate::support::{Options, PASSWORD, TestApp, TestRequest, user};

async fn enable_totp(app: &TestApp, token: &str) -> (Vec<u8>, Vec<String>) {
    let setup = app
        .send(TestRequest::post("/api/v1/me/mfa/totp").session(token))
        .await;
    let secret = crate::support::base32_decode(&setup.decode::<v1::TotpSetup>().secret);
    let confirmed = app
        .send(
            TestRequest::post("/api/v1/me/mfa/totp/confirm")
                .session(token)
                .proto(&v1::CodeRequest {
                    code: app.totp(&secret),
                }),
        )
        .await;
    assert_eq!(confirmed.status, StatusCode::OK, "{}", confirmed.text);
    let codes = confirmed
        .decode::<v1::SecondFactorAdded>()
        .recovery_codes
        .unwrap()
        .codes;
    (secret, codes)
}

async fn first_step(app: &TestApp, email: &str) -> String {
    let first = app.login(email, PASSWORD).await;
    assert_eq!(first.status, StatusCode::ACCEPTED, "{}", first.text);
    first.cookie("mfa").unwrap()
}

fn answer(path: &str, attempt: &str, code: &str) -> TestRequest {
    TestRequest::post(path)
        .cookie("mfa", attempt)
        .proto(&v1::CodeRequest {
            code: code.to_owned(),
        })
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn a_recovery_code_finishes_the_sign_in_once(pool: PgPool) {
    let app = TestApp::new(pool);
    let token = app.register_verified("alice@example.com").await;
    let (_, codes) = enable_totp(&app, &token).await;

    let attempt = first_step(&app, "alice@example.com").await;
    let signed_in = app
        .send(answer(
            "/api/v1/auth/mfa/recovery-code",
            &attempt,
            &codes[0],
        ))
        .await;
    assert_eq!(signed_in.status, StatusCode::OK, "{}", signed_in.text);
    assert_eq!(user(&signed_in.decode()).email, "alice@example.com");
    assert_eq!(signed_in.cookie("mfa").as_deref(), Some(""));
    app.me(&signed_in.session_token().unwrap()).await;

    let attempt = first_step(&app, "alice@example.com").await;
    app.send(answer(
        "/api/v1/auth/mfa/recovery-code",
        &attempt,
        &codes[0],
    ))
    .await
    .assert_invalid_code();
    let security = app
        .send(TestRequest::get("/api/v1/me/security").session(&token))
        .await
        .decode::<v1::SecurityOverview>();
    assert_eq!(security.recovery_codes_remaining, 9);
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn recovery_codes_are_replaced_and_the_app_removed(pool: PgPool) {
    let app = TestApp::new(pool);
    let token = app.register_verified("alice@example.com").await;
    let (secret, old) = enable_totp(&app, &token).await;

    let fresh = app
        .send(TestRequest::post("/api/v1/me/mfa/recovery-codes").session(&token))
        .await;
    assert_eq!(fresh.status, StatusCode::OK, "{}", fresh.text);
    let fresh = fresh.decode::<v1::RecoveryCodes>().codes;
    assert_eq!(fresh.len(), 10);
    assert!(fresh.iter().all(|code| !old.contains(code)));

    let attempt = first_step(&app, "alice@example.com").await;
    app.send(answer("/api/v1/auth/mfa/recovery-code", &attempt, &old[1]))
        .await
        .assert_invalid_code();
    let signed_in = app
        .send(answer(
            "/api/v1/auth/mfa/recovery-code",
            &attempt,
            &fresh[0],
        ))
        .await;
    assert_eq!(signed_in.status, StatusCode::OK, "{}", signed_in.text);

    let remove = |code: &str| {
        TestRequest::delete("/api/v1/me/mfa/totp")
            .session(&token)
            .proto(&v1::CodeRequest {
                code: code.to_owned(),
            })
    };
    app.clock.advance(Duration::seconds(30));
    app.send(remove("000000")).await.assert_invalid_code();
    let removed = app.send(remove(&app.totp(&secret))).await;
    assert_eq!(removed.status, StatusCode::NO_CONTENT, "{}", removed.text);

    let login = app.login("alice@example.com", PASSWORD).await;
    assert_eq!(login.status, StatusCode::OK, "{}", login.text);
    app.send(remove(&app.totp(&secret)))
        .await
        .assert_problem(StatusCode::NOT_FOUND, "not_found");
    app.send(TestRequest::post("/api/v1/me/mfa/recovery-codes").session(&token))
        .await
        .assert_problem(StatusCode::CONFLICT, "mfa_disabled");
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn answers_are_limited_per_account_however_many_attempts_are_started(pool: PgPool) {
    use std::num::NonZeroU32;

    use api::rate_limit::{Rate, Rates};

    let generous = Rate::new(
        NonZeroU32::new(1000).unwrap(),
        std::time::Duration::from_secs(60),
    );
    let tight = Rate::new(
        NonZeroU32::new(4).unwrap(),
        std::time::Duration::from_hours(4),
    );
    let app = TestApp::with(
        pool,
        Options {
            rates: Some(Rates {
                api_per_ip: None,
                login_per_ip: generous,
                login_per_account: generous,
                login_per_account_total: generous,
                register_per_ip: generous,
                password_reset_per_ip: generous,
                password_reset_per_account: generous,
                verification_per_ip: generous,
                verification_per_account: generous,
                send_code_per_ip: generous,
                send_code_per_account: generous,
                check_code_per_ip: generous,
                check_code_per_account: tight,
                ceremony_per_ip: generous,
                report_per_ip: generous,
                upload_per_ip: generous,
                upload_per_account: generous,
            }),
            ..Options::default()
        },
    );
    let token = app.register_verified("alice@example.com").await;
    let (secret, _) = enable_totp(&app, &token).await;

    for _ in 0..3 {
        let attempt = first_step(&app, "alice@example.com").await;
        let wrong = app
            .send(answer("/api/v1/auth/mfa/totp", &attempt, "000000"))
            .await;
        assert_eq!(
            wrong.status,
            StatusCode::UNPROCESSABLE_ENTITY,
            "{}",
            wrong.text
        );
    }
    let attempt = first_step(&app, "alice@example.com").await;
    app.clock.advance(Duration::seconds(30));
    app.send(answer(
        "/api/v1/auth/mfa/totp",
        &attempt,
        &app.totp(&secret),
    ))
    .await
    .assert_problem(StatusCode::TOO_MANY_REQUESTS, "rate_limited");
}

struct Authenticator {
    key: Ed25519KeyPair,
    credential_id: Vec<u8>,
}

impl Authenticator {
    fn new() -> Self {
        Self {
            key: Ed25519KeyPair::from_seed_unchecked(&[42; 32]).unwrap(),
            credential_id: b"software-key".to_vec(),
        }
    }

    fn spki(&self) -> Vec<u8> {
        // DER `SubjectPublicKeyInfo` for Ed25519, then the 32-byte key.
        let mut spki = vec![
            0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00,
        ];
        spki.extend_from_slice(self.key.public_key().as_ref());
        spki
    }

    fn client_data(kind: &str, challenge: &[u8]) -> Vec<u8> {
        use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
        format!(
            r#"{{"type":"{kind}","challenge":"{}","origin":"{}"}}"#,
            URL_SAFE_NO_PAD.encode(challenge),
            crate::support::ORIGIN
        )
        .into_bytes()
    }

    fn authenticator_data(&self, flags: u8, sign_count: u32, attested: bool) -> Vec<u8> {
        let mut data = digest::digest(&digest::SHA256, b"localhost")
            .as_ref()
            .to_vec();
        data.push(flags);
        data.extend_from_slice(&sign_count.to_be_bytes());
        if attested {
            data.extend_from_slice(&[0; 16]);
            data.extend_from_slice(
                &u16::try_from(self.credential_id.len())
                    .unwrap()
                    .to_be_bytes(),
            );
            data.extend_from_slice(&self.credential_id);
        }
        data
    }

    fn register(&self, options: &v1::PasskeyCreationOptions) -> v1::RegisterPasskeyRequest {
        let challenge = &options.public_key.as_ref().unwrap().challenge;
        v1::RegisterPasskeyRequest {
            challenge_id: options.challenge_id.clone(),
            name: "Software key".to_owned(),
            credential_id: self.credential_id.clone(),
            client_data_json: Self::client_data("webauthn.create", challenge),
            authenticator_data: self.authenticator_data(0x45, 0, true),
            public_key: self.spki(),
            public_key_algorithm: -8,
            transports: vec!["internal".to_owned()],
        }
    }

    fn assert(
        &self,
        challenge_id: &str,
        challenge: &[u8],
        flags: u8,
        sign_count: u32,
    ) -> v1::PasskeyAssertionRequest {
        let client = Self::client_data("webauthn.get", challenge);
        let data = self.authenticator_data(flags, sign_count, false);
        let mut message = data.clone();
        message.extend_from_slice(digest::digest(&digest::SHA256, &client).as_ref());
        v1::PasskeyAssertionRequest {
            challenge_id: challenge_id.to_owned(),
            credential_id: self.credential_id.clone(),
            client_data_json: client,
            authenticator_data: data,
            signature: self.key.sign(&message).as_ref().to_vec(),
            user_handle: None,
        }
    }
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn a_passkey_signs_in_alone_and_as_the_second_step(pool: PgPool) {
    let app = TestApp::new(pool);
    let token = app.register_verified("alice@example.com").await;
    let key = Authenticator::new();

    let options = app
        .send(TestRequest::post("/api/v1/me/passkeys/options").session(&token))
        .await
        .decode::<v1::PasskeyCreationOptions>();
    let registered = app
        .send(
            TestRequest::post("/api/v1/me/passkeys")
                .session(&token)
                .proto(&key.register(&options)),
        )
        .await;
    assert_eq!(
        registered.status,
        StatusCode::CREATED,
        "{}",
        registered.text
    );

    let options = app
        .send(TestRequest::post("/api/v1/auth/passkeys/options"))
        .await
        .decode::<v1::PasskeyRequestOptions>();
    let challenge = options.public_key.unwrap().challenge;
    let signed_in = app
        .send(
            TestRequest::post("/api/v1/auth/passkeys/login").proto(&key.assert(
                &options.challenge_id,
                &challenge,
                0x05,
                1,
            )),
        )
        .await;
    assert_eq!(signed_in.status, StatusCode::OK, "{}", signed_in.text);
    app.me(&signed_in.session_token().unwrap()).await;

    let first = app.login("alice@example.com", PASSWORD).await;
    assert_eq!(first.status, StatusCode::ACCEPTED, "{}", first.text);
    assert!(
        first
            .decode::<v1::MfaChallenge>()
            .methods()
            .any(|method| method == v1::MfaMethod::Passkey)
    );
    let attempt = first.cookie("mfa").unwrap();
    let options = app
        .send(TestRequest::post("/api/v1/auth/mfa/passkeys/options").cookie("mfa", &attempt))
        .await;
    assert_eq!(options.status, StatusCode::OK, "{}", options.text);
    let options = options.decode::<v1::PasskeyRequestOptions>();
    let challenge = options.public_key.unwrap().challenge;
    let second = app
        .send(
            TestRequest::post("/api/v1/auth/mfa/passkeys")
                .cookie("mfa", &attempt)
                .proto(&key.assert(&options.challenge_id, &challenge, 0x01, 2)),
        )
        .await;
    assert_eq!(second.status, StatusCode::OK, "{}", second.text);
    assert_eq!(second.cookie("mfa").as_deref(), Some(""));
    assert_eq!(user(&second.decode()).email, "alice@example.com");
}
