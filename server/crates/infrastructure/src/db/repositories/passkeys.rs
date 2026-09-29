//! `PasskeyRepository`: registered WebAuthn credentials and their short-lived challenges.
//!
//! A credential id is unique across users (`passkeys_credential_id_key`). Challenges are consumed
//! with `delete ... returning`, so each one can be answered at most once.

use domain::{
    error::{StorageError, UnknownValue},
    passkey::{
        ChallengeId, ChallengePurpose, NewPasskey, Passkey, PasskeyId, PasskeyName,
        PasskeyRepository, PublicKeyAlgorithm, WebAuthnChallenge,
    },
    user::UserId,
};
use time::OffsetDateTime;
use uuid::Uuid;

use super::{CLEANUP_BATCH, cleanup_done};
use crate::db::{
    errors::{corrupt, db_error, to_u64},
    postgres::{PgExecutor, PgHandle},
};

struct PasskeyRow {
    id: Uuid,
    user_id: Uuid,
    credential_id: Vec<u8>,
    public_key: Vec<u8>,
    algorithm: i32,
    sign_count: i64,
    transports: Vec<String>,
    name: String,
    created_at: OffsetDateTime,
    last_used_at: Option<OffsetDateTime>,
}

impl TryFrom<PasskeyRow> for Passkey {
    type Error = StorageError;

    fn try_from(row: PasskeyRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: PasskeyId::from_uuid(row.id),
            user_id: UserId::from_uuid(row.user_id),
            credential_id: row.credential_id,
            public_key: row.public_key,
            algorithm: PublicKeyAlgorithm::from_cose(i64::from(row.algorithm)).ok_or_else(
                || {
                    corrupt(UnknownValue::new(
                        "COSE algorithm",
                        row.algorithm.to_string(),
                    ))
                },
            )?,
            sign_count: u32::try_from(row.sign_count).map_err(corrupt)?,
            transports: row.transports,
            name: PasskeyName::parse(&row.name).map_err(corrupt)?,
            created_at: row.created_at,
            last_used_at: row.last_used_at,
        })
    }
}

fn cose(algorithm: PublicKeyAlgorithm) -> i32 {
    // The COSE ids used here (-7, -8, -257) all fit.
    i32::try_from(algorithm.cose()).unwrap_or_default()
}

impl<C: PgHandle> PasskeyRepository for PgExecutor<C> {
    async fn create_passkey(&mut self, passkey: &NewPasskey) -> Result<Passkey, StorageError> {
        sqlx::query_as!(
            PasskeyRow,
            r#"
            insert into passkeys
                (id, user_id, credential_id, public_key, algorithm, sign_count, transports, name)
            values ($1, $2, $3, $4, $5, $6, $7, $8)
            returning id, user_id, credential_id, public_key, algorithm, sign_count, transports,
                name, created_at, last_used_at
            "#,
            passkey.id.as_uuid(),
            passkey.user_id.as_uuid(),
            passkey.credential_id,
            passkey.public_key,
            cose(passkey.algorithm),
            i64::from(passkey.sign_count),
            &passkey.transports,
            passkey.name.as_str(),
        )
        .fetch_one(self.conn())
        .await
        .map_err(db_error)?
        .try_into()
    }

    async fn list_user_passkeys(&mut self, user: UserId) -> Result<Vec<Passkey>, StorageError> {
        sqlx::query_as!(
            PasskeyRow,
            r#"
            select id, user_id, credential_id, public_key, algorithm, sign_count, transports,
                name, created_at, last_used_at
            from passkeys
            where user_id = $1
            order by created_at, id
            "#,
            user.as_uuid(),
        )
        .fetch_all(self.conn())
        .await
        .map_err(db_error)?
        .into_iter()
        .map(Passkey::try_from)
        .collect()
    }

    async fn find_passkey_by_credential(
        &mut self,
        credential_id: &[u8],
    ) -> Result<Option<Passkey>, StorageError> {
        sqlx::query_as!(
            PasskeyRow,
            r#"
            select id, user_id, credential_id, public_key, algorithm, sign_count, transports,
                name, created_at, last_used_at
            from passkeys
            where credential_id = $1
            "#,
            credential_id,
        )
        .fetch_optional(self.conn())
        .await
        .map_err(db_error)?
        .map(Passkey::try_from)
        .transpose()
    }

    async fn record_passkey_use(
        &mut self,
        id: PasskeyId,
        sign_count: u32,
        at: OffsetDateTime,
    ) -> Result<(), StorageError> {
        sqlx::query!(
            "update passkeys set sign_count = $2, last_used_at = $3 where id = $1",
            id.as_uuid(),
            i64::from(sign_count),
            at,
        )
        .execute(self.conn())
        .await
        .map_err(db_error)?;
        Ok(())
    }

    async fn rename_user_passkey(
        &mut self,
        user: UserId,
        id: PasskeyId,
        name: &PasskeyName,
    ) -> Result<Option<Passkey>, StorageError> {
        sqlx::query_as!(
            PasskeyRow,
            r#"
            update passkeys set name = $3
            where id = $2 and user_id = $1
            returning id, user_id, credential_id, public_key, algorithm, sign_count, transports,
                name, created_at, last_used_at
            "#,
            user.as_uuid(),
            id.as_uuid(),
            name.as_str(),
        )
        .fetch_optional(self.conn())
        .await
        .map_err(db_error)?
        .map(Passkey::try_from)
        .transpose()
    }

    async fn delete_user_passkey(
        &mut self,
        user: UserId,
        id: PasskeyId,
    ) -> Result<bool, StorageError> {
        let result = sqlx::query!(
            "delete from passkeys where id = $2 and user_id = $1",
            user.as_uuid(),
            id.as_uuid(),
        )
        .execute(self.conn())
        .await
        .map_err(db_error)?;
        Ok(result.rows_affected() > 0)
    }

    async fn delete_user_passkeys(&mut self, user: UserId) -> Result<u64, StorageError> {
        let result = sqlx::query!("delete from passkeys where user_id = $1", user.as_uuid())
            .execute(self.conn())
            .await
            .map_err(db_error)?;
        Ok(result.rows_affected())
    }

    async fn count_user_passkeys(&mut self, user: UserId) -> Result<u64, StorageError> {
        let count = sqlx::query_scalar!(
            r#"select count(*) as "count!" from passkeys where user_id = $1"#,
            user.as_uuid(),
        )
        .fetch_one(self.conn())
        .await
        .map_err(db_error)?;
        to_u64(count)
    }

    async fn create_webauthn_challenge(
        &mut self,
        challenge: &WebAuthnChallenge,
    ) -> Result<(), StorageError> {
        sqlx::query!(
            r#"
            insert into webauthn_challenges (id, challenge, purpose, user_id, expires_at)
            values ($1, $2, $3, $4, $5)
            "#,
            challenge.id.as_uuid(),
            challenge.challenge,
            challenge.purpose.as_str(),
            challenge.user_id.map(|id| id.as_uuid()),
            challenge.expires_at,
        )
        .execute(self.conn())
        .await
        .map_err(db_error)?;
        Ok(())
    }

    async fn consume_webauthn_challenge(
        &mut self,
        id: ChallengeId,
        purpose: ChallengePurpose,
        now: OffsetDateTime,
    ) -> Result<Option<WebAuthnChallenge>, StorageError> {
        let row = sqlx::query!(
            r#"
            delete from webauthn_challenges
            where id = $1 and purpose = $2 and expires_at > $3
            returning id, challenge, purpose, user_id, expires_at
            "#,
            id.as_uuid(),
            purpose.as_str(),
            now,
        )
        .fetch_optional(self.conn())
        .await
        .map_err(db_error)?;

        row.map(|row| {
            Ok(WebAuthnChallenge {
                id: ChallengeId::from_uuid(row.id),
                challenge: row.challenge,
                purpose: ChallengePurpose::parse(&row.purpose).map_err(corrupt)?,
                user_id: row.user_id.map(UserId::from_uuid),
                expires_at: row.expires_at,
            })
        })
        .transpose()
    }

    async fn delete_expired_webauthn_challenges(
        &mut self,
        now: OffsetDateTime,
    ) -> Result<u64, StorageError> {
        let mut total = 0;
        loop {
            let deleted = sqlx::query!(
                r#"
                delete from webauthn_challenges
                where id in (
                    select id from webauthn_challenges where expires_at <= $1 limit $2
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
