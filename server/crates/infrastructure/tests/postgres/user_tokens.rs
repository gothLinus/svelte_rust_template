use domain::{
    clock::truncate_to_micros,
    secret::TokenHash,
    user_token::{TokenPolicy, TokenPurpose, UserToken, UserTokenRepository},
};
use sqlx::PgPool;
use time::{Duration, OffsetDateTime};

use crate::support::{conn, user};

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn tokens_are_single_use(pool: PgPool) {
    let mut conn = conn(&pool).await;
    let alice = user(&mut conn, "alice@example.com").await;
    let now = truncate_to_micros(OffsetDateTime::now_utc());
    let token = UserToken::issue(
        alice.id(),
        TokenPurpose::PasswordReset,
        TokenHash::new([1; 32]),
        now,
        &TokenPolicy::default(),
    );
    conn.replace_user_token(&token).await.unwrap();

    assert!(
        conn.consume_user_token(&token.token_hash, TokenPurpose::EmailVerification, now)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        conn.consume_user_token(&token.token_hash, TokenPurpose::PasswordReset, now)
            .await
            .unwrap()
            .map(|consumed| consumed.user_id),
        Some(alice.id())
    );
    assert!(
        conn.consume_user_token(&token.token_hash, TokenPurpose::PasswordReset, now)
            .await
            .unwrap()
            .is_none()
    );
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn a_new_token_replaces_the_previous_one(pool: PgPool) {
    let mut conn = conn(&pool).await;
    let alice = user(&mut conn, "alice@example.com").await;
    let now = truncate_to_micros(OffsetDateTime::now_utc());
    let issue = |byte: u8, purpose| {
        UserToken::issue(
            alice.id(),
            purpose,
            TokenHash::new([byte; 32]),
            now,
            &TokenPolicy::default(),
        )
    };

    conn.replace_user_token(&issue(1, TokenPurpose::PasswordReset))
        .await
        .unwrap();
    conn.replace_user_token(&issue(2, TokenPurpose::EmailVerification))
        .await
        .unwrap();
    conn.replace_user_token(&issue(3, TokenPurpose::PasswordReset))
        .await
        .unwrap();

    let reset = TokenPurpose::PasswordReset;
    assert!(
        conn.consume_user_token(&TokenHash::new([1; 32]), reset, now)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        conn.consume_user_token(&TokenHash::new([3; 32]), reset, now)
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(
        conn.delete_user_tokens(alice.id(), TokenPurpose::EmailVerification)
            .await
            .unwrap(),
        1
    );
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn expired_tokens_cannot_be_used_and_are_cleaned_up(pool: PgPool) {
    let mut conn = conn(&pool).await;
    let alice = user(&mut conn, "alice@example.com").await;
    let now = truncate_to_micros(OffsetDateTime::now_utc());
    let token = UserToken::issue(
        alice.id(),
        TokenPurpose::EmailVerification,
        TokenHash::new([1; 32]),
        now - Duration::days(2),
        &TokenPolicy::default(),
    );
    conn.replace_user_token(&token).await.unwrap();

    assert!(
        conn.consume_user_token(&token.token_hash, token.purpose, now)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(conn.delete_expired_user_tokens(now).await.unwrap(), 1);
}
