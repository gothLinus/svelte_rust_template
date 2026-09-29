//! `MfaRepository`: TOTP credentials, recovery codes and login challenges.
//!
//! The checks that stop replays and brute force are single statements, so concurrent requests
//! cannot both pass them: `use_totp_step` only accepts a time step greater than the last one used,
//! `consume_recovery_code` deletes the code, and `reserve_mfa_attempt` counts the guess and checks
//! the limit in the same `update`.

use domain::{
    error::StorageError,
    mfa::{MAX_MFA_ATTEMPTS, MfaChallenge, MfaRepository, TotpCredential},
    secret::TokenHash,
    user::UserId,
};
use time::OffsetDateTime;

use super::{CLEANUP_BATCH, cleanup_done};
use crate::db::{
    errors::{corrupt, db_error, to_u64},
    postgres::{PgExecutor, PgHandle},
};

impl<C: PgHandle> MfaRepository for PgExecutor<C> {
    async fn find_totp(&mut self, user: UserId) -> Result<Option<TotpCredential>, StorageError> {
        let row = sqlx::query!(
            r#"
            select user_id, sealed_secret, confirmed_at, last_used_step
            from totp_credentials
            where user_id = $1
            "#,
            user.as_uuid(),
        )
        .fetch_optional(self.conn())
        .await
        .map_err(db_error)?;

        Ok(row.map(|row| TotpCredential {
            user_id: UserId::from_uuid(row.user_id),
            sealed_secret: row.sealed_secret,
            confirmed_at: row.confirmed_at,
            last_used_step: row.last_used_step,
        }))
    }

    async fn start_totp_setup(
        &mut self,
        user: UserId,
        sealed_secret: &[u8],
        at: OffsetDateTime,
    ) -> Result<bool, StorageError> {
        let result = sqlx::query!(
            r#"
            insert into totp_credentials (user_id, sealed_secret, created_at)
            values ($1, $2, $3)
            on conflict (user_id) do update
            set sealed_secret = excluded.sealed_secret,
                last_used_step = null,
                created_at = excluded.created_at
            where totp_credentials.confirmed_at is null
            "#,
            user.as_uuid(),
            sealed_secret,
            at,
        )
        .execute(self.conn())
        .await
        .map_err(db_error)?;
        Ok(result.rows_affected() > 0)
    }

    async fn delete_stale_totp_setups(
        &mut self,
        cutoff: OffsetDateTime,
    ) -> Result<u64, StorageError> {
        let result = sqlx::query!(
            "delete from totp_credentials where confirmed_at is null and created_at < $1",
            cutoff,
        )
        .execute(self.conn())
        .await
        .map_err(db_error)?;
        Ok(result.rows_affected())
    }

    async fn confirm_totp(
        &mut self,
        user: UserId,
        at: OffsetDateTime,
        step: i64,
    ) -> Result<bool, StorageError> {
        let result = sqlx::query!(
            r#"
            update totp_credentials set confirmed_at = $2, last_used_step = $3
            where user_id = $1 and confirmed_at is null
            "#,
            user.as_uuid(),
            at,
            step,
        )
        .execute(self.conn())
        .await
        .map_err(db_error)?;
        Ok(result.rows_affected() > 0)
    }

    async fn use_totp_step(&mut self, user: UserId, step: i64) -> Result<bool, StorageError> {
        let result = sqlx::query!(
            r#"
            update totp_credentials set last_used_step = $2
            where user_id = $1
              and confirmed_at is not null
              and (last_used_step is null or last_used_step < $2)
            "#,
            user.as_uuid(),
            step,
        )
        .execute(self.conn())
        .await
        .map_err(db_error)?;
        Ok(result.rows_affected() > 0)
    }

    async fn delete_totp(&mut self, user: UserId) -> Result<bool, StorageError> {
        let result = sqlx::query!(
            "delete from totp_credentials where user_id = $1",
            user.as_uuid()
        )
        .execute(self.conn())
        .await
        .map_err(db_error)?;
        Ok(result.rows_affected() > 0)
    }

    async fn replace_recovery_codes(
        &mut self,
        user: UserId,
        code_hashes: &[TokenHash],
    ) -> Result<(), StorageError> {
        sqlx::query!(
            "delete from recovery_codes where user_id = $1",
            user.as_uuid()
        )
        .execute(&mut *self.conn())
        .await
        .map_err(db_error)?;

        let hashes: Vec<Vec<u8>> = code_hashes
            .iter()
            .map(|hash| hash.as_bytes().to_vec())
            .collect();
        sqlx::query!(
            r#"
            insert into recovery_codes (user_id, code_hash)
            select $1, hash from unnest($2::bytea[]) as hash
            "#,
            user.as_uuid(),
            &hashes,
        )
        .execute(self.conn())
        .await
        .map_err(db_error)?;
        Ok(())
    }

    async fn consume_recovery_code(
        &mut self,
        user: UserId,
        code_hash: &TokenHash,
    ) -> Result<bool, StorageError> {
        let result = sqlx::query!(
            "delete from recovery_codes where user_id = $1 and code_hash = $2",
            user.as_uuid(),
            code_hash.as_bytes().as_slice(),
        )
        .execute(self.conn())
        .await
        .map_err(db_error)?;
        Ok(result.rows_affected() > 0)
    }

    async fn count_recovery_codes(&mut self, user: UserId) -> Result<u64, StorageError> {
        let count = sqlx::query_scalar!(
            r#"select count(*) as "count!" from recovery_codes where user_id = $1"#,
            user.as_uuid(),
        )
        .fetch_one(self.conn())
        .await
        .map_err(db_error)?;
        to_u64(count)
    }

    async fn create_mfa_challenge(&mut self, challenge: &MfaChallenge) -> Result<(), StorageError> {
        sqlx::query!(
            r#"
            insert into mfa_challenges (token_hash, user_id, attempts, expires_at)
            values ($1, $2, 0, $3)
            "#,
            challenge.token_hash.as_bytes().as_slice(),
            challenge.user_id.as_uuid(),
            challenge.expires_at,
        )
        .execute(self.conn())
        .await
        .map_err(db_error)?;
        Ok(())
    }

    async fn find_mfa_challenge(
        &mut self,
        token_hash: &TokenHash,
    ) -> Result<Option<MfaChallenge>, StorageError> {
        let row = sqlx::query!(
            r#"
            select token_hash, user_id, attempts, expires_at
            from mfa_challenges
            where token_hash = $1
            "#,
            token_hash.as_bytes().as_slice(),
        )
        .fetch_optional(self.conn())
        .await
        .map_err(db_error)?;

        row.map(|row| {
            Ok(MfaChallenge {
                token_hash: TokenHash::from_slice(&row.token_hash).map_err(corrupt)?,
                user_id: UserId::from_uuid(row.user_id),
                attempts: u32::try_from(row.attempts).map_err(corrupt)?,
                expires_at: row.expires_at,
            })
        })
        .transpose()
    }

    async fn reserve_mfa_attempt(
        &mut self,
        token_hash: &TokenHash,
        now: OffsetDateTime,
    ) -> Result<Option<MfaChallenge>, StorageError> {
        let row = sqlx::query!(
            r#"
            update mfa_challenges set attempts = attempts + 1
            where token_hash = $1 and attempts < $2 and expires_at > $3
            returning token_hash, user_id, attempts, expires_at
            "#,
            token_hash.as_bytes().as_slice(),
            i32::try_from(MAX_MFA_ATTEMPTS).map_err(corrupt)?,
            now,
        )
        .fetch_optional(self.conn())
        .await
        .map_err(db_error)?;

        row.map(|row| {
            Ok(MfaChallenge {
                token_hash: TokenHash::from_slice(&row.token_hash).map_err(corrupt)?,
                user_id: UserId::from_uuid(row.user_id),
                attempts: u32::try_from(row.attempts).map_err(corrupt)?,
                expires_at: row.expires_at,
            })
        })
        .transpose()
    }

    async fn delete_mfa_challenge(&mut self, token_hash: &TokenHash) -> Result<bool, StorageError> {
        let result = sqlx::query!(
            "delete from mfa_challenges where token_hash = $1",
            token_hash.as_bytes().as_slice(),
        )
        .execute(self.conn())
        .await
        .map_err(db_error)?;
        Ok(result.rows_affected() > 0)
    }

    async fn delete_user_mfa_challenges(&mut self, user: UserId) -> Result<u64, StorageError> {
        let result = sqlx::query!(
            "delete from mfa_challenges where user_id = $1",
            user.as_uuid()
        )
        .execute(self.conn())
        .await
        .map_err(db_error)?;
        Ok(result.rows_affected())
    }

    async fn delete_expired_mfa_challenges(
        &mut self,
        now: OffsetDateTime,
    ) -> Result<u64, StorageError> {
        let mut total = 0;
        loop {
            let deleted = sqlx::query!(
                r#"
                delete from mfa_challenges
                where token_hash in (
                    select token_hash from mfa_challenges where expires_at <= $1 limit $2
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
