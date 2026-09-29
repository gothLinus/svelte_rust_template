//! `OneTimeCodeRepository`: short numeric codes sent by email or text.
//!
//! There is at most one code per `(user, purpose)`; issuing a new one replaces the old one and
//! resets its attempt counter. `reserve_code_attempt` counts a guess and checks the limit and
//! expiry in a single `update`, so parallel guesses cannot exceed `MAX_CODE_ATTEMPTS`.

use domain::{
    error::StorageError,
    one_time_code::{
        CodeChannel, CodePurpose, MAX_CODE_ATTEMPTS, OneTimeCode, OneTimeCodeRepository,
    },
    secret::TokenHash,
    user::{PhoneNumber, UserId},
};
use time::OffsetDateTime;
use uuid::Uuid;

use super::{CLEANUP_BATCH, cleanup_done};
use crate::db::{
    errors::{corrupt, db_error},
    postgres::{PgExecutor, PgHandle},
};

struct CodeRow {
    user_id: Uuid,
    purpose: String,
    channel: String,
    code_hash: Vec<u8>,
    target: Option<String>,
    attempts: i32,
    expires_at: OffsetDateTime,
}

impl TryFrom<CodeRow> for OneTimeCode {
    type Error = StorageError;

    fn try_from(row: CodeRow) -> Result<Self, Self::Error> {
        Ok(Self {
            user_id: UserId::from_uuid(row.user_id),
            purpose: CodePurpose::parse(&row.purpose).map_err(corrupt)?,
            channel: CodeChannel::parse(&row.channel).map_err(corrupt)?,
            code_hash: TokenHash::from_slice(&row.code_hash).map_err(corrupt)?,
            target: row
                .target
                .as_deref()
                .map(PhoneNumber::parse)
                .transpose()
                .map_err(corrupt)?,
            attempts: u32::try_from(row.attempts).map_err(corrupt)?,
            expires_at: row.expires_at,
        })
    }
}

impl<C: PgHandle> OneTimeCodeRepository for PgExecutor<C> {
    async fn replace_one_time_code(&mut self, code: &OneTimeCode) -> Result<(), StorageError> {
        sqlx::query!(
            r#"
            insert into one_time_codes
                (user_id, purpose, channel, code_hash, target, attempts, expires_at)
            values ($1, $2, $3, $4, $5, 0, $6)
            on conflict (user_id, purpose) do update
            set channel = excluded.channel,
                code_hash = excluded.code_hash,
                target = excluded.target,
                attempts = 0,
                expires_at = excluded.expires_at,
                created_at = now()
            "#,
            code.user_id.as_uuid(),
            code.purpose.as_str(),
            code.channel.as_str(),
            code.code_hash.as_bytes().as_slice(),
            code.target.as_ref().map(PhoneNumber::as_str),
            code.expires_at,
        )
        .execute(self.conn())
        .await
        .map_err(db_error)?;
        Ok(())
    }

    async fn find_one_time_code(
        &mut self,
        user: UserId,
        purpose: CodePurpose,
    ) -> Result<Option<OneTimeCode>, StorageError> {
        sqlx::query_as!(
            CodeRow,
            r#"
            select user_id, purpose, channel, code_hash, target, attempts, expires_at
            from one_time_codes
            where user_id = $1 and purpose = $2
            "#,
            user.as_uuid(),
            purpose.as_str(),
        )
        .fetch_optional(self.conn())
        .await
        .map_err(db_error)?
        .map(OneTimeCode::try_from)
        .transpose()
    }

    async fn reserve_code_attempt(
        &mut self,
        user: UserId,
        purpose: CodePurpose,
        now: OffsetDateTime,
    ) -> Result<Option<OneTimeCode>, StorageError> {
        sqlx::query_as!(
            CodeRow,
            r#"
            update one_time_codes set attempts = attempts + 1
            where user_id = $1 and purpose = $2 and attempts < $3 and expires_at > $4
            returning user_id, purpose, channel, code_hash, target, attempts, expires_at
            "#,
            user.as_uuid(),
            purpose.as_str(),
            i32::try_from(MAX_CODE_ATTEMPTS).map_err(corrupt)?,
            now,
        )
        .fetch_optional(self.conn())
        .await
        .map_err(db_error)?
        .map(OneTimeCode::try_from)
        .transpose()
    }

    async fn delete_one_time_code(
        &mut self,
        user: UserId,
        purpose: CodePurpose,
    ) -> Result<bool, StorageError> {
        let result = sqlx::query!(
            "delete from one_time_codes where user_id = $1 and purpose = $2",
            user.as_uuid(),
            purpose.as_str(),
        )
        .execute(self.conn())
        .await
        .map_err(db_error)?;
        Ok(result.rows_affected() > 0)
    }

    async fn delete_expired_one_time_codes(
        &mut self,
        now: OffsetDateTime,
    ) -> Result<u64, StorageError> {
        let mut total = 0;
        loop {
            let deleted = sqlx::query!(
                r#"
                delete from one_time_codes
                where (user_id, purpose) in (
                    select user_id, purpose from one_time_codes where expires_at <= $1 limit $2
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
