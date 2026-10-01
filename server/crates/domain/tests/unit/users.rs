use domain::{
    secret::Secret,
    user::{
        Email, LoginIdentifier, MAX_PASSWORD_LEN, MAX_USERNAME_LEN, NewPassword, PasswordHash,
        PhoneNumber, Username, is_common_password,
    },
};

#[test]
fn email_is_trimmed_and_lowercased() {
    let email = Email::parse("  Alice@Example.COM ").unwrap();
    assert_eq!(email.as_str(), "alice@example.com");
    assert_eq!(email.to_string(), "alice@example.com");
}

#[test]
fn email_rejects_malformed_addresses() {
    for raw in [
        "alice",
        "@example.com",
        "alice@",
        "alice@example",
        "alice@.example.com",
        "alice@example.com.",
        "alice@exa..mple.com",
        "al ice@example.com",
        "alice@@example.com",
        "alice@exa@mple.com",
    ] {
        let err = Email::parse(raw).unwrap_err();
        assert_eq!(err.code(), "invalid_email", "{raw}");
    }
}

#[test]
fn blank_email_is_required() {
    assert_eq!(Email::parse("   ").unwrap_err().code(), "required");
}

#[test]
fn email_has_a_length_limit() {
    let long = format!("{}@example.com", "a".repeat(250));
    assert!(Email::parse(&long).is_err());
}

#[test]
fn email_debug_and_mask_hide_the_local_part() {
    let email = Email::parse("alice@example.com").unwrap();
    assert_eq!(email.masked(), "a***@example.com");
    assert_eq!(format!("{email:?}"), "Email(a***@example.com)");
}

#[test]
fn new_password_policy() {
    let parse = |raw: &str| NewPassword::parse(Secret::new(raw));

    assert_eq!(parse("").unwrap_err().code(), "required");
    assert_eq!(parse("short").unwrap_err().code(), "too_short");
    assert_eq!(parse("        ").unwrap_err().code(), "too_weak");
    assert_eq!(
        parse(&"x".repeat(MAX_PASSWORD_LEN + 1)).unwrap_err().code(),
        "too_long"
    );
    assert!(parse("correct horse").is_ok());
    // Characters, not bytes, count towards the minimum.
    assert!(parse("ääääääää").is_ok());
    assert!(parse(&"x".repeat(MAX_PASSWORD_LEN)).is_ok());
    assert!(parse("xyzzy-42").is_ok());
    assert_eq!(parse("xyzzy-4").unwrap_err().code(), "too_short");
}

#[test]
fn breached_passwords_are_refused_in_any_case() {
    let parse = |raw: &str| NewPassword::parse(Secret::new(raw));
    for common in [
        "password",
        "Password1",
        "12345678",
        "iloveyou",
        "QWERTYUIOP",
    ] {
        assert_eq!(parse(common).unwrap_err().code(), "too_common", "{common}");
    }
    assert!(is_common_password("PASSWORD"));
    assert!(!is_common_password("correct horse battery"));
    // The list only holds what the length policy would let through.
    assert!(!is_common_password("123456"));
}

#[test]
fn secrets_and_hashes_are_redacted_from_debug() {
    let password = NewPassword::parse(Secret::new("correct horse")).unwrap();
    assert_eq!(format!("{password:?}"), "NewPassword(<redacted>)");
    assert_eq!(password.as_secret().expose(), "correct horse");

    let hash = PasswordHash::new("$argon2id$v=19$secret");
    assert_eq!(format!("{hash:?}"), "PasswordHash(<redacted>)");
    assert_eq!(hash.as_str(), "$argon2id$v=19$secret");
}

#[test]
fn usernames_are_lowercased_handles_with_a_letter() {
    assert_eq!(Username::parse("  Alice_99 ").unwrap().as_str(), "alice_99");
    assert_eq!(Username::parse("a.b-c").unwrap().as_str(), "a.b-c");

    for (raw, code) in [
        ("", "required"),
        ("ab", "too_short"),
        (&"a".repeat(31), "too_long"),
        ("12345", "invalid_username"),
        ("_alice", "invalid_username"),
        ("al ice", "invalid_username"),
        ("alice@home", "invalid_username"),
        ("jürgen", "invalid_username"),
    ] {
        assert_eq!(Username::parse(raw).unwrap_err().code(), code, "{raw}");
    }
}

#[test]
fn usernames_are_suggested_from_free_text() {
    let suggest = |hint: &str, suffix: &str| Username::suggest(hint, suffix).map(|u| u.to_string());

    assert_eq!(
        suggest("Alice.Liddell", "").as_deref(),
        Some("alice.liddell")
    );
    assert_eq!(suggest("Jürgen Klopp", "").as_deref(), Some("jrgenklopp"));
    assert_eq!(suggest("__bob", "0042").as_deref(), Some("bob0042"));
    assert_eq!(suggest("12345", ""), None);
    assert_eq!(suggest("李", ""), None);

    let long = suggest(&"a".repeat(40), "1234").unwrap();
    assert_eq!(long.len(), MAX_USERNAME_LEN);
    assert!(long.ends_with("1234"));
}

#[test]
fn phone_numbers_are_normalized_to_e164() {
    let phone = PhoneNumber::parse(" +49 (170) 123-45.67 ").unwrap();
    assert_eq!(phone.as_str(), "+491701234567");
    assert_eq!(phone.masked(), "+49••••••••67");
    assert!(!format!("{phone:?}").contains("1234"));

    for raw in [
        "0170 1234567",
        "+0123456789",
        "+12",
        "+1234567890123456",
        "+49 abc",
    ] {
        assert_eq!(
            PhoneNumber::parse(raw).unwrap_err().code(),
            "invalid_phone",
            "{raw}"
        );
    }
    assert_eq!(PhoneNumber::parse(" ").unwrap_err().code(), "required");
}

#[test]
fn login_identifiers_are_told_apart_by_shape() {
    assert!(matches!(
        LoginIdentifier::parse("Alice@Example.com").unwrap(),
        LoginIdentifier::Email(email) if email.as_str() == "alice@example.com"
    ));
    assert!(matches!(
        LoginIdentifier::parse("alice").unwrap(),
        LoginIdentifier::Username(name) if name.as_str() == "alice"
    ));
    assert!(matches!(
        LoginIdentifier::parse("+49 170 1234567").unwrap(),
        LoginIdentifier::Phone(phone) if phone.as_str() == "+491701234567"
    ));
    assert!(matches!(
        LoginIdentifier::parse("0170 1234567").unwrap_err().code(),
        "invalid_phone"
    ));
    assert_eq!(LoginIdentifier::parse("  ").unwrap_err().code(), "required");
}

#[test]
fn user_status_follows_its_timestamps() {
    use domain::user::{User, UserId, UserParts};
    use time::macros::datetime;

    let at = datetime!(2026-01-01 12:00 UTC);
    let parts = || UserParts {
        id: UserId::generate(),
        email: Email::parse("alice@example.com").unwrap(),
        username: Username::parse("alice").unwrap(),
        phone: None,
        phone_verified_at: None,
        password_hash: None,
        email_verified_at: None,
        disabled_at: None,
        locale: None,
        created_at: at,
        updated_at: at,
    };

    let fresh = User::from_parts(parts());
    assert!(!fresh.is_disabled());
    assert!(!fresh.is_email_verified());
    assert!(!fresh.has_password());

    let settled = User::from_parts(UserParts {
        email_verified_at: Some(at),
        disabled_at: Some(at),
        password_hash: Some(PasswordHash::new("$argon2id$x")),
        ..parts()
    });
    assert!(settled.is_disabled());
    assert!(settled.is_email_verified());
    assert!(settled.has_password());
    assert!(settled.locale().is_none());

    let german = User::from_parts(UserParts {
        locale: domain::i18n::Locale::parse("de"),
        ..parts()
    });
    assert_eq!(
        german.locale().map(domain::i18n::Locale::as_str),
        Some("de")
    );
}

#[test]
fn calling_codes_are_checked_and_cover_their_numbers() {
    use domain::user::CallingCode;

    let swiss = CallingCode::parse(" +41 ").unwrap();
    assert_eq!(swiss.as_str(), "+41");
    assert!(swiss.covers(&PhoneNumber::parse("+41 79 123 45 67").unwrap()));
    assert!(!swiss.covers(&PhoneNumber::parse("+49 170 1234567").unwrap()));
    for invalid in ["41", "+0", "+1234", "+4a", "DE", ""] {
        assert_eq!(
            CallingCode::parse(invalid).unwrap_err().code(),
            "invalid_calling_code",
            "{invalid}"
        );
    }
}
