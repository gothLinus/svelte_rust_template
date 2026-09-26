use domain::secret::{MAX_SECRET_BYTES, Secret, TokenHash};

#[test]
fn secret_is_redacted_from_debug() {
    let secret = Secret::new("hunter2");
    assert_eq!(format!("{secret:?}"), "Secret(<redacted>)");
    assert_eq!(secret.expose(), "hunter2");
    assert!(!secret.is_empty());
    assert!(Secret::default().is_empty());
}

#[test]
fn secret_size_limit() {
    assert!(Secret::new("a".repeat(MAX_SECRET_BYTES)).is_within_limit());
    assert!(!Secret::new("a".repeat(MAX_SECRET_BYTES + 1)).is_within_limit());
}

#[test]
fn token_hash_requires_32_bytes() {
    let hash = TokenHash::from_slice(&[7; 32]).unwrap();
    assert_eq!(hash.as_bytes(), &[7; 32]);
    assert_eq!(format!("{hash:?}"), "TokenHash(<redacted>)");
    assert_eq!(
        TokenHash::from_slice(&[7; 31]).unwrap_err().to_string(),
        "unknown token hash length `31`"
    );
}

#[test]
fn constant_time_eq_compares_every_byte_and_the_length() {
    use domain::secret::constant_time_eq;

    assert!(constant_time_eq(b"", b""));
    assert!(constant_time_eq(b"same bytes", b"same bytes"));
    assert!(!constant_time_eq(b"same bytes", b"Same bytes"));
    assert!(!constant_time_eq(b"same bytes", b"same byteS"));
    assert!(!constant_time_eq(b"same", b"same bytes"));
    assert!(!constant_time_eq(b"same bytes", b"same"));
}

#[test]
fn token_hashes_are_equal_only_when_every_byte_is() {
    let mut other = [7; 32];
    assert_eq!(TokenHash::new([7; 32]), TokenHash::new([7; 32]));
    other[31] = 8;
    assert_ne!(TokenHash::new([7; 32]), TokenHash::new(other));
    other = [7; 32];
    other[0] = 0;
    assert_ne!(TokenHash::new([7; 32]), TokenHash::new(other));
}
