//! `UserTokenRepository`: single-use emailed tokens, one `TokenPurpose` each (email verification,
//! password reset, magic link, ...). Only the token's digest is stored.
//!
//! There is at most one token per `(user, purpose)`; issuing a new one replaces the old.
//! `consume_user_token` is a single `delete ... returning`, so two requests racing with the same
//! link cannot both succeed.

use domain::{
    error::StorageError,
    secret::TokenHash,
    user::{Email, UserId},
    user_token::{ConsumedToken, TokenPurpose, UserToken, UserTokenRepository},
};
use time::OffsetDateTime;

use super::{CLEANUP_BATCH, cleanup_done};
use crate::db::{
    errors::{corrupt, db_error},
    postgres::{PgExecutor, PgHandle},
};

impl<C: PgHandle> UserTokenRepository for PgExecutor<C> {
    async fn replace_user_token(&mut self, token: &UserToken) -> Result<(), StorageError> {
        sqlx::query!(
            r#"
            insert into user_tokens (token_hash, user_id, purpose, new_email, expires_at)
            values ($1, $2, $3, $4, $5)
            on conflict on constraint user_tokens_user_id_purpose_key do update
            set token_hash = excluded.token_hash,
                new_email = excluded.new_email,
                expires_at = excluded.expires_at,
                created_at = now()
            "#,
            token.token_hash.as_bytes().as_slice(),
            token.user_id.as_uuid(),
            token.purpose.as_str(),
            token.email.as_ref().map(Email::as_str),
            token.expires_at,
        )
        .execute(self.conn())
        .await
        .map_err(db_error)?;

        Ok(())
    }

    async fn consume_user_token(
        &mut self,
        token_hash: &TokenHash,
        purpose: TokenPurpose,
        now: OffsetDateTime,
    ) -> Result<Option<ConsumedToken>, StorageError> {
        let row = sqlx::query!(
            r#"
            delete from user_tokens
            where token_hash = $1 and purpose = $2 and expires_at > $3
            returning user_id, new_email
            "#,
            token_hash.as_bytes().as_slice(),
            purpose.as_str(),
            now,
        )
        .fetch_optional(self.conn())
        .await
        .map_err(db_error)?;

        row.map(|row| {
            Ok(ConsumedToken {
                user_id: UserId::from_uuid(row.user_id),
                email: row
                    .new_email
                    .as_deref()
                    .map(Email::parse)
                    .transpose()
                    .map_err(corrupt)?,
            })
        })
        .transpose()
    }

    async fn user_token_exists(
        &mut self,
        token_hash: &TokenHash,
        purpose: TokenPurpose,
        now: OffsetDateTime,
    ) -> Result<bool, StorageError> {
        sqlx::query_scalar!(
            r#"
            select exists (
                select 1 from user_tokens
                where token_hash = $1 and purpose = $2 and expires_at > $3
            ) as "exists!"
            "#,
            token_hash.as_bytes().as_slice(),
            purpose.as_str(),
            now,
        )
        .fetch_one(self.conn())
        .await
        .map_err(db_error)
    }

    async fn delete_user_tokens(
        &mut self,
        user: UserId,
        purpose: TokenPurpose,
    ) -> Result<u64, StorageError> {
        let result = sqlx::query!(
            "delete from user_tokens where user_id = $1 and purpose = $2",
            user.as_uuid(),
            purpose.as_str(),
        )
        .execute(self.conn())
        .await
        .map_err(db_error)?;

        Ok(result.rows_affected())
    }

    async fn delete_expired_user_tokens(
        &mut self,
        now: OffsetDateTime,
    ) -> Result<u64, StorageError> {
        let mut total = 0;
        loop {
            let deleted = sqlx::query!(
                r#"
                delete from user_tokens
                where token_hash in (
                    select token_hash from user_tokens where expires_at <= $1 limit $2
                )
                and expires_at <= $1
                "#,
                now,
                CLEANUP_BATCH,
            )
            .execute(self.conn())
            .await
            .map_err(db_error)?
            .rows_affected();
            total += deleted;
            if cleanup_done(deleted) {
                return Ok(total);
            }
        }
    }
}
