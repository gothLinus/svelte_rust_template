use application::mfa::totp::{base32, matching_step};
use domain::{
    mfa::{TOTP_PERIOD_SECONDS, hotp_code, totp_step},
    security::Crypto,
};
use time::{Duration, OffsetDateTime};

use crate::support::fakes::FakeCrypto;

#[test]
fn setup_keys_are_rfc_4648_base32_without_padding() {
    for (input, encoded) in [
        ("", ""),
        ("f", "MY"),
        ("fo", "MZXQ"),
        ("foo", "MZXW6"),
        ("foob", "MZXW6YQ"),
        ("fooba", "MZXW6YTB"),
        ("foobar", "MZXW6YTBOI"),
    ] {
        assert_eq!(base32(input.as_bytes()), encoded, "{input:?}");
    }
    assert_eq!(base32(&[0; 5]), "AAAAAAAA");
    assert_eq!(base32(&[0xff; 5]), "77777777");
    assert_eq!(base32(&[0x5a; 20]).len(), 32);
}

fn code_at(secret: &[u8], at: OffsetDateTime) -> String {
    let step = totp_step(at);
    hotp_code(&FakeCrypto::default().hmac_sha1(secret, &step.to_be_bytes()))
}

#[test]
fn codes_are_accepted_one_step_either_side_and_no_further() {
    let crypto = FakeCrypto::default();
    let secret = b"12345678901234567890";
    let now = OffsetDateTime::from_unix_timestamp(1_111_111_109).unwrap();
    let step = Duration::seconds(TOTP_PERIOD_SECONDS);
    let current = totp_step(now);

    for (offset, accepted) in [(-2, false), (-1, true), (0, true), (1, true), (2, false)] {
        let code = code_at(secret, now + step * offset);
        let matched = matching_step(&crypto, secret, &code, now);
        if accepted {
            assert_eq!(
                matched,
                Some(current + i64::from(offset)),
                "offset {offset}"
            );
        } else {
            assert_eq!(matched, None, "offset {offset}");
        }
    }
}

#[test]
fn codes_may_be_typed_with_separators_but_not_with_another_length() {
    let crypto = FakeCrypto::default();
    let secret = b"12345678901234567890";
    let now = OffsetDateTime::from_unix_timestamp(1_234_567_890).unwrap();
    let code = code_at(secret, now);

    let spaced = format!("{} {}", &code[..3], &code[3..]);
    assert!(matching_step(&crypto, secret, &spaced, now).is_some());
    assert!(matching_step(&crypto, secret, &code[..5], now).is_none());
    assert!(matching_step(&crypto, secret, &format!("{code}0"), now).is_none());
    assert!(matching_step(&crypto, b"another secret", &code, now).is_none());
}
