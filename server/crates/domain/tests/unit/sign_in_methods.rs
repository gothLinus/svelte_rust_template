use domain::{
    mfa::{MAX_MFA_ATTEMPTS, MFA_CHALLENGE_TTL, MfaChallenge, hotp_code, totp_step},
    one_time_code::{CODE_TTL, CodeChannel, CodePurpose, MAX_CODE_ATTEMPTS, OneTimeCode},
    passkey::{
        ChallengePurpose, MAX_PASSKEY_NAME_LEN, Passkey, PasskeyId, PasskeyName, PublicKeyAlgorithm,
    },
    secret::TokenHash,
    user::UserId,
};
use time::{Duration, macros::datetime};

fn hex(raw: &str) -> [u8; 20] {
    let mut out = [0u8; 20];
    for (i, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&raw[i * 2..i * 2 + 2], 16).unwrap();
    }
    out
}

#[test]
fn hotp_truncation_matches_rfc_4226() {
    // RFC 4226 appendix D: HMAC-SHA1("12345678901234567890", counter) and the resulting codes.
    for (mac, code) in [
        ("cc93cf18508d94934c64b65d8ba7667fb7cde4b0", "755224"),
        ("75a48a19d4cbe100644e8ac1397eea747a2d33ab", "287082"),
        ("0bacb7fa082fef30782211938bc1c5e70416ff44", "359152"),
        ("66c28227d03a2d5529262ff016a1e6ef76557ece", "969429"),
    ] {
        assert_eq!(hotp_code(&hex(mac)), code);
    }
}

#[test]
fn totp_steps_are_thirty_seconds() {
    assert_eq!(totp_step(datetime!(1970-01-01 00:00:59 UTC)), 1);
    assert_eq!(totp_step(datetime!(1970-01-01 00:01:00 UTC)), 2);
    assert_eq!(totp_step(datetime!(2005-03-18 01:58:29 UTC)), 37_037_036);
}

#[test]
fn mfa_challenges_expire_and_run_out_of_attempts() {
    let now = datetime!(2026-01-01 00:00 UTC);
    let mut challenge = MfaChallenge::start(UserId::generate(), TokenHash::new([0; 32]), now);
    assert!(challenge.is_live(now));
    assert!(!challenge.is_live(now + MFA_CHALLENGE_TTL));

    challenge.attempts = MAX_MFA_ATTEMPTS;
    assert!(!challenge.is_live(now));
}

#[test]
fn one_time_codes_expire_and_run_out_of_attempts() {
    let now = datetime!(2026-01-01 00:00 UTC);
    let mut code = OneTimeCode::issue(
        UserId::generate(),
        CodePurpose::Login,
        CodeChannel::Email,
        TokenHash::new([0; 32]),
        now,
    );
    assert!(code.is_live(now + CODE_TTL - Duration::SECOND));
    assert!(!code.is_live(now + CODE_TTL));
    code.attempts = MAX_CODE_ATTEMPTS;
    assert!(!code.is_live(now));
}

#[test]
fn enums_round_trip() {
    for purpose in CodePurpose::ALL {
        assert_eq!(CodePurpose::parse(purpose.as_str()).unwrap(), purpose);
    }
    for channel in CodeChannel::ALL {
        assert_eq!(CodeChannel::parse(channel.as_str()).unwrap(), channel);
        assert_eq!(channel.to_string(), channel.as_str());
    }
    for purpose in ChallengePurpose::ALL {
        assert_eq!(ChallengePurpose::parse(purpose.as_str()).unwrap(), purpose);
    }
    for alg in PublicKeyAlgorithm::ALL {
        assert_eq!(PublicKeyAlgorithm::from_cose(alg.cose()), Some(alg));
    }
    assert!(CodePurpose::parse("x").is_err());
    assert!(CodeChannel::parse("pigeon").is_err());
    assert!(ChallengePurpose::parse("x").is_err());
    assert_eq!(PublicKeyAlgorithm::from_cose(-35), None);
}

#[test]
fn passkey_names() {
    assert_eq!(
        PasskeyName::parse("  MacBook ").unwrap().as_str(),
        "MacBook"
    );
    assert_eq!(PasskeyName::parse("").unwrap_err().code(), "required");
    assert_eq!(
        PasskeyName::parse(&"x".repeat(MAX_PASSKEY_NAME_LEN + 1))
            .unwrap_err()
            .code(),
        "too_long"
    );
    assert_eq!(
        PasskeyName::parse("a\u{7}b").unwrap_err().code(),
        "invalid_characters"
    );
}

#[test]
fn a_signature_counter_must_go_up_unless_the_authenticator_has_none() {
    let passkey = |sign_count| Passkey {
        id: PasskeyId::generate(),
        user_id: UserId::generate(),
        credential_id: vec![1],
        public_key: vec![2],
        algorithm: PublicKeyAlgorithm::Es256,
        sign_count,
        transports: Vec::new(),
        name: PasskeyName::parse("key").unwrap(),
        created_at: datetime!(2026-01-01 00:00 UTC),
        last_used_at: None,
    };
    assert!(passkey(0).accepts_sign_count(0));
    assert!(passkey(0).accepts_sign_count(1));
    assert!(passkey(5).accepts_sign_count(6));
    assert!(!passkey(5).accepts_sign_count(5));
    assert!(!passkey(5).accepts_sign_count(0));
}
