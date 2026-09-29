use base64::{
    Engine,
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
};
use domain::{
    identity::{AuthorizationRequest, IdentityProviders, OAuthError},
    one_time_code::CodeChannel,
    passkey::PublicKeyAlgorithm,
    security::Crypto,
    text::{TextMessage, TextSender},
    user::PhoneNumber,
};
use infrastructure::{
    config::{ClientSecret, OAuthConfig, OAuthProviderConfig, PublicOrigin},
    crypto::RingCrypto,
    oauth::{AppleKey, OAuthProviders, Provider, http_client, id_token::Expected},
    testing::{RecordingTexts, claims_from_token_endpoint},
    text::{DisabledTextSender, LogTextSender},
};
use ring::{
    rand::SystemRandom,
    signature::{
        ECDSA_P256_SHA256_ASN1_SIGNING, ECDSA_P256_SHA256_FIXED_SIGNING, EcdsaKeyPair,
        Ed25519KeyPair, KeyPair,
    },
};
use time::OffsetDateTime;

fn crypto() -> RingCrypto {
    RingCrypto::new(&[42; 32])
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut out, b| {
        write!(out, "{b:02x}").unwrap();
        out
    })
}

#[test]
fn sealing_round_trips_and_detects_tampering() {
    let crypto = crypto();
    let sealed = crypto.seal(b"authenticator seed", b"totp:alice").unwrap();
    assert_ne!(
        sealed,
        crypto.seal(b"authenticator seed", b"totp:alice").unwrap()
    );
    assert_eq!(
        &**crypto.open(&sealed, b"totp:alice").unwrap(),
        b"authenticator seed"
    );

    let mut tampered = sealed.clone();
    *tampered.last_mut().unwrap() ^= 1;
    assert!(crypto.open(&tampered, b"totp:alice").is_err());
    assert!(crypto.open(&sealed[..5], b"totp:alice").is_err());
    assert!(
        RingCrypto::new(&[1; 32])
            .open(&sealed, b"totp:alice")
            .is_err()
    );
    assert!(crypto.open(&sealed, b"totp:bob").is_err());
    assert!(!format!("{crypto:?}").contains("42"));
}

#[test]
fn a_rotated_key_still_opens_what_the_previous_one_sealed() {
    let old = RingCrypto::new(&[1; 32]);
    let sealed = old.seal(b"seed", b"ctx").unwrap();
    let rotated = RingCrypto::new(&[2; 32]).with_previous_key(&[1; 32]);
    assert_eq!(&**rotated.open(&sealed, b"ctx").unwrap(), b"seed");
    let resealed = rotated.seal(b"seed", b"ctx").unwrap();
    assert!(old.open(&resealed, b"ctx").is_err());

    assert_eq!(
        rotated.previous_keyed_digest("codes", b"x"),
        Some(old.keyed_digest("codes", b"x"))
    );
    assert_eq!(old.previous_keyed_digest("codes", b"x"), None);
}

#[test]
fn values_sealed_in_version_1_still_open_and_unversioned_ones_do_not() {
    use ring::aead::{AES_256_GCM, Aad, LessSafeKey, Nonce, UnboundKey};

    let crypto = RingCrypto::new(&[1; 32]);
    let sealed = crypto.seal(b"seed", b"ctx").unwrap();
    assert_eq!(sealed[0], 2, "new values use version 2");
    let fingerprint = &sealed[1..5];

    // Version 1: `[1][fingerprint][nonce][ciphertext]`, sealed with the raw key.
    let key = LessSafeKey::new(UnboundKey::new(&AES_256_GCM, &[1; 32]).unwrap());
    let nonce = [7u8; 12];
    let mut ciphertext = b"old seed".to_vec();
    key.seal_in_place_append_tag(
        Nonce::assume_unique_for_key(nonce),
        Aad::from(b"ctx"),
        &mut ciphertext,
    )
    .unwrap();
    let v1 = [&[1u8][..], fingerprint, &nonce, &ciphertext].concat();
    assert_eq!(&**crypto.open(&v1, b"ctx").unwrap(), b"old seed");

    // Version 2 does not use the raw key: relabelling a v1 value does not open it.
    let mut relabelled = v1.clone();
    relabelled[0] = 2;
    assert!(crypto.open(&relabelled, b"ctx").is_err());

    let mut plain = b"older seed".to_vec();
    key.seal_in_place_append_tag(
        Nonce::assume_unique_for_key(nonce),
        Aad::empty(),
        &mut plain,
    )
    .unwrap();
    let unversioned = [nonce.as_slice(), &plain].concat();
    assert!(crypto.open(&unversioned, b"any").is_err());
}

#[test]
fn keyed_digests_depend_on_the_key_and_the_purpose() {
    let a = RingCrypto::new(&[1; 32]);
    let b = RingCrypto::new(&[2; 32]);
    assert_eq!(
        a.keyed_digest("codes", b"123456"),
        a.keyed_digest("codes", b"123456")
    );
    assert_ne!(
        a.keyed_digest("codes", b"123456"),
        b.keyed_digest("codes", b"123456")
    );
    assert_ne!(
        a.keyed_digest("codes", b"123456"),
        a.keyed_digest("other", b"123456")
    );
    // Not the plain hash, which a dump of six-digit codes would invert at once.
    assert_ne!(a.keyed_digest("codes", b"123456"), a.sha256(b"123456"));
}

#[test]
fn hashes_match_their_standards() {
    let crypto = crypto();
    assert_eq!(
        hex(&crypto.sha256(b"abc")),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        hex(&crypto.hmac_sha1(b"12345678901234567890", &0u64.to_be_bytes())),
        "cc93cf18508d94934c64b65d8ba7667fb7cde4b0"
    );
}

#[test]
fn randomness() {
    let crypto = crypto();
    let code = crypto.random_digits(6).unwrap();
    assert_eq!(code.expose().len(), 6);
    assert!(code.expose().chars().all(|c| c.is_ascii_digit()));
    assert_eq!(crypto.random_bytes(20).unwrap().len(), 20);
    assert_ne!(
        crypto.random_bytes(16).unwrap(),
        crypto.random_bytes(16).unwrap()
    );
}

const P256_SPKI_PREFIX: &str = "3059301306072a8648ce3d020106082a8648ce3d030107034200";
const ED25519_SPKI_PREFIX: &str = "302a300506032b6570032100";

fn unhex(raw: &str) -> Vec<u8> {
    (0..raw.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&raw[i..i + 2], 16).unwrap())
        .collect()
}

#[test]
fn passkey_signatures_verify_with_the_spki_key() {
    let crypto = crypto();
    let rng = SystemRandom::new();

    let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, &rng).unwrap();
    let pair =
        EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, pkcs8.as_ref(), &rng).unwrap();
    let mut spki = unhex(P256_SPKI_PREFIX);
    spki.extend_from_slice(pair.public_key().as_ref());
    let signature = pair.sign(&rng, b"message").unwrap();

    assert!(crypto.is_valid_public_key(PublicKeyAlgorithm::Es256, &spki));
    assert!(crypto.verify_signature(
        PublicKeyAlgorithm::Es256,
        &spki,
        b"message",
        signature.as_ref()
    ));
    assert!(!crypto.verify_signature(
        PublicKeyAlgorithm::Es256,
        &spki,
        b"other",
        signature.as_ref()
    ));
    assert!(!crypto.is_valid_public_key(PublicKeyAlgorithm::EdDsa, &spki));
    assert!(!crypto.is_valid_public_key(PublicKeyAlgorithm::Rs256, &spki));

    let pkcs8 = Ed25519KeyPair::generate_pkcs8(&rng).unwrap();
    let pair = Ed25519KeyPair::from_pkcs8(pkcs8.as_ref()).unwrap();
    let mut spki = unhex(ED25519_SPKI_PREFIX);
    spki.extend_from_slice(pair.public_key().as_ref());
    let signature = pair.sign(b"message");
    assert!(crypto.verify_signature(
        PublicKeyAlgorithm::EdDsa,
        &spki,
        b"message",
        signature.as_ref()
    ));

    for garbage in [&b""[..], b"\x30\x03\x02\x01\x00", &spki[..10]] {
        assert!(!crypto.is_valid_public_key(PublicKeyAlgorithm::Es256, garbage));
        assert!(!crypto.verify_signature(PublicKeyAlgorithm::Es256, garbage, b"m", b"s"));
    }
    let mut trailing = spki.clone();
    trailing.push(0);
    assert!(!crypto.is_valid_public_key(PublicKeyAlgorithm::EdDsa, &trailing));
}

#[test]
fn apple_keys_are_checked_up_front_and_sign_client_secrets() {
    let rng = SystemRandom::new();
    let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng).unwrap();
    let pem = format!(
        "-----BEGIN PRIVATE KEY-----\\n{}\\n-----END PRIVATE KEY-----",
        STANDARD.encode(pkcs8.as_ref())
    );
    let key = AppleKey::new("TEAM".to_owned(), "KEY".to_owned(), &pem).unwrap();
    assert!(!format!("{key:?}").contains(&STANDARD.encode(pkcs8.as_ref())[..20]));

    assert!(AppleKey::new("T".to_owned(), "K".to_owned(), "nope").is_err());
    assert!(
        AppleKey::new(
            "T".to_owned(),
            "K".to_owned(),
            &STANDARD.encode(b"not a key")
        )
        .is_err()
    );
}

#[tokio::test]
async fn text_transports() {
    assert!(DisabledTextSender.channels().is_empty());
    assert_eq!(
        LogTextSender.channels(),
        [CodeChannel::Sms, CodeChannel::Whatsapp]
    );

    let message = TextMessage {
        to: PhoneNumber::parse("+15551234567").unwrap(),
        channel: CodeChannel::Sms,
        body: "123 456 is your code".to_owned(),
        valid_for: domain::one_time_code::CODE_TTL,
    };
    assert!(!format!("{message:?}").contains("123 456"));
    LogTextSender.send(message.clone()).await.unwrap();
    DisabledTextSender.send(message.clone()).await.unwrap();

    let recording = RecordingTexts::new();
    recording.send(message).await.unwrap();
    assert_eq!(
        recording.last_code_to("+15551234567").as_deref(),
        Some("123456")
    );
    assert_eq!(recording.sent().len(), 1);
}

fn providers() -> OAuthProviders {
    let config = OAuthConfig {
        providers: vec![
            OAuthProviderConfig {
                provider: Provider::Github,
                client_id: "gh-client".to_owned(),
                secret: ClientSecret::Shared(domain::secret::Secret::new("s")),
                microsoft_tenant: None,
            },
            OAuthProviderConfig {
                provider: Provider::Google,
                client_id: "g-client".to_owned(),
                secret: ClientSecret::Shared(domain::secret::Secret::new("s")),
                microsoft_tenant: None,
            },
        ],
    };
    OAuthProviders::new(
        &config,
        PublicOrigin::parse("https://app.example.com").unwrap(),
        http_client().unwrap(),
    )
}

#[test]
fn authorization_urls_carry_state_pkce_and_nonce() {
    let providers = providers();
    assert_eq!(providers.providers()[0].name, "GitHub");
    assert_eq!(
        providers.redirect_uri(Provider::Github),
        "https://app.example.com/api/v1/auth/oauth/github/callback"
    );

    let request = |provider| AuthorizationRequest {
        provider,
        state: "the-state",
        nonce: "the-nonce",
        pkce_challenge: "the-challenge",
    };
    let github = providers.authorization_url(&request("github")).unwrap();
    assert!(github.starts_with("https://github.com/login/oauth/authorize?"));
    for part in [
        "client_id=gh-client",
        "state=the-state",
        "code_challenge=the-challenge",
        "code_challenge_method=S256",
        "redirect_uri=https%3A%2F%2Fapp.example.com%2Fapi%2Fv1%2Fauth%2Foauth%2Fgithub%2Fcallback",
    ] {
        assert!(github.contains(part), "{github}");
    }
    assert!(!github.contains("nonce"));

    let google = providers.authorization_url(&request("google")).unwrap();
    assert!(google.contains("nonce=the-nonce") && google.contains("scope=openid+email+profile"));

    assert!(matches!(
        providers.authorization_url(&request("myspace")),
        Err(OAuthError::UnknownProvider)
    ));
    assert_eq!(Provider::Apple.to_string(), "Apple");
    assert!(
        Provider::ALL
            .iter()
            .all(|p| p.client_id_var().starts_with("OAUTH_")
                && p.client_secret_var().starts_with("OAUTH_"))
    );
}

fn jwt(claims: &serde_json::Value) -> String {
    format!(
        "{}.{}.sig",
        URL_SAFE_NO_PAD.encode(br#"{"alg":"RS256"}"#),
        URL_SAFE_NO_PAD.encode(claims.to_string())
    )
}

#[test]
fn id_tokens_are_checked_for_issuer_audience_expiry_and_nonce() {
    let now = OffsetDateTime::now_utc().unix_timestamp();
    let apple = |iss: &str| iss == "https://appleid.apple.com";
    let expected = Expected {
        issuer: &apple,
        client_id: "client",
        nonce: "n",
    };
    let good = serde_json::json!({
        "iss": "https://appleid.apple.com",
        "aud": "client",
        "exp": now + 60,
        "sub": "001",
        "nonce": "n",
        "email": "a@privaterelay.appleid.com",
        "email_verified": "true",
    });
    let claims = claims_from_token_endpoint(&jwt(&good), &expected).unwrap();
    assert_eq!(claims.subject, "001");
    assert!(claims.email_verified);

    let with = |key: &str, value: serde_json::Value| {
        let mut claims = good.clone();
        claims[key] = value;
        claims_from_token_endpoint(&jwt(&claims), &expected)
    };
    assert!(with("aud", serde_json::json!(["other", "client"])).is_err());
    let mut shared = good.clone();
    shared["aud"] = serde_json::json!(["other", "client"]);
    shared["azp"] = "client".into();
    assert!(claims_from_token_endpoint(&jwt(&shared), &expected).is_ok());
    shared["azp"] = "other".into();
    assert!(claims_from_token_endpoint(&jwt(&shared), &expected).is_err());
    assert!(with("azp", "other".into()).is_err());
    assert!(with("iss", "https://evil.test".into()).is_err());
    assert!(with("aud", "other".into()).is_err());
    assert!(with("exp", (now - 3600).into()).is_err());
    assert!(with("nonce", "m".into()).is_err());
    assert!(!with("email_verified", false.into()).unwrap().email_verified);
    assert!(claims_from_token_endpoint("not-a-jwt", &expected).is_err());
    assert!(claims_from_token_endpoint("a.!!!.c", &expected).is_err());
}
