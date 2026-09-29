use domain::{
    secret::TokenHash,
    user::{Email, UserId},
    user_token::{TokenPolicy, TokenPurpose, UserToken},
};
use time::{Duration, macros::datetime};

#[test]
fn purposes_round_trip() {
    for purpose in TokenPurpose::ALL {
        assert_eq!(TokenPurpose::parse(purpose.as_str()).unwrap(), purpose);
        assert_eq!(purpose.to_string(), purpose.as_str());
    }
    assert!(TokenPurpose::parse("login").is_err());
    // Naming every variant makes adding one without listing it in `ALL` a compile error here.
    for purpose in TokenPurpose::ALL {
        match purpose {
            TokenPurpose::EmailVerification
            | TokenPurpose::PasswordReset
            | TokenPurpose::EmailChange
            | TokenPurpose::MagicLink
            | TokenPurpose::EmailChangeCancel => {}
        }
    }
    assert_eq!(TokenPurpose::ALL.len(), 5);
}

#[test]
fn tokens_expire_after_their_purposes_ttl() {
    let policy = TokenPolicy {
        email_verification_ttl: Duration::hours(24),
        password_reset_ttl: Duration::minutes(30),
        email_change_ttl: Duration::hours(12),
        magic_link_ttl: Duration::minutes(15),
    };
    let now = datetime!(2026-01-01 00:00 UTC);
    let user = UserId::generate();

    let verify = UserToken::issue(
        user,
        TokenPurpose::EmailVerification,
        TokenHash::new([0; 32]),
        now,
        &policy,
    );
    let reset = UserToken::issue(
        user,
        TokenPurpose::PasswordReset,
        TokenHash::new([1; 32]),
        now,
        &policy,
    );

    assert_eq!(verify.expires_at, now + Duration::hours(24));
    assert_eq!(reset.expires_at, now + Duration::minutes(30));
    assert_eq!(reset.user_id, user);
}

#[test]
fn reset_links_are_short_lived_by_default() {
    let policy = TokenPolicy::default();
    assert!(policy.password_reset_ttl <= Duration::hours(1));
    assert!(policy.email_verification_ttl > policy.password_reset_ttl);
}

#[test]
fn email_change_tokens_carry_the_new_address() {
    let policy = TokenPolicy::default();
    let now = datetime!(2026-01-01 00:00 UTC);
    let email = Email::parse("new@example.com").unwrap();

    let token = UserToken::email_change(
        UserId::generate(),
        email.clone(),
        TokenHash::new([2; 32]),
        now,
        &policy,
    );
    let magic = UserToken::issue(
        UserId::generate(),
        TokenPurpose::MagicLink,
        TokenHash::new([3; 32]),
        now,
        &policy,
    );

    assert_eq!(token.purpose, TokenPurpose::EmailChange);
    assert_eq!(token.email, Some(email));
    assert_eq!(token.expires_at, now + policy.email_change_ttl);
    assert_eq!(magic.email, None);
    assert!(policy.magic_link_ttl <= Duration::minutes(30));

    let old = Email::parse("old@example.com").unwrap();
    let cancel = UserToken::email_change_cancel(
        UserId::generate(),
        old.clone(),
        TokenHash::new([4; 32]),
        now,
        &policy,
    );
    assert_eq!(cancel.purpose, TokenPurpose::EmailChangeCancel);
    assert_eq!(cancel.email, Some(old));
    assert_eq!(cancel.expires_at, now + policy.email_change_ttl);
}
