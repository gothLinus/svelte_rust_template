//! `IdentityRepository`: external (OAuth) identities and the in-flight sign-in flows.
//!
//! An identity is unique per `(provider, subject)` and per `(user, provider)`; the violations
//! surface as `StorageError::UniqueViolation` with those constraint names. A flow is consumed with
//! `delete ... returning`, so a callback can only be handled once.

use domain::{
    error::StorageError,
    identity::{ExternalIdentity, IdentityId, IdentityRepository, NewIdentity, OAuthFlow},
    secret::{Secret, TokenHash},
    user::UserId,
};
use time::OffsetDateTime;
use uuid::Uuid;

use super::{CLEANUP_BATCH, cleanup_done};
use crate::db::{
    errors::{corrupt, db_error},
    postgres::{PgExecutor, PgHandle},
};

struct IdentityRow {
    id: Uuid,
    user_id: Uuid,
    provider: String,
    subject: String,
    email: Option<String>,
    created_at: OffsetDateTime,
    last_used_at: OffsetDateTime,
}

impl From<IdentityRow> for ExternalIdentity {
    fn from(row: IdentityRow) -> Self {
        Self {
            id: IdentityId::from_uuid(row.id),
            user_id: UserId::from_uuid(row.user_id),
            provider: row.provider,
            subject: row.subject,
            email: row.email,
            created_at: row.created_at,
            last_used_at: row.last_used_at,
        }
    }
}

impl<C: PgHandle> IdentityRepository for PgExecutor<C> {
    async fn create_identity(
        &mut self,
        identity: &NewIdentity,
    ) -> Result<ExternalIdentity, StorageError> {
        sqlx::query_as!(
            IdentityRow,
            r#"
            insert into external_identities (id, user_id, provider, subject, email)
            values ($1, $2, $3, $4, $5)
            returning id, user_id, provider, subject, email, created_at, last_used_at
            "#,
            identity.id.as_uuid(),
            identity.user_id.as_uuid(),
            identity.provider,
            identity.subject,
            identity.email,
        )
        .fetch_one(self.conn())
        .await
        .map(ExternalIdentity::from)
        .map_err(db_error)
    }

    async fn find_identity(
        &mut self,
        provider: &str,
        subject: &str,
    ) -> Result<Option<ExternalIdentity>, StorageError> {
        sqlx::query_as!(
            IdentityRow,
            r#"
            select id, user_id, provider, subject, email, created_at, last_used_at
            from external_identities
            where provider = $1 and subject = $2
            "#,
            provider,
            subject,
        )
        .fetch_optional(self.conn())
        .await
        .map(|row| row.map(ExternalIdentity::from))
        .map_err(db_error)
    }

    async fn list_user_identities(
        &mut self,
        user: UserId,
    ) -> Result<Vec<ExternalIdentity>, StorageError> {
        sqlx::query_as!(
            IdentityRow,
            r#"
            select id, user_id, provider, subject, email, created_at, last_used_at
            from external_identities
            where user_id = $1
            order by created_at, id
            "#,
            user.as_uuid(),
        )
        .fetch_all(self.conn())
        .await
        .map(|rows| rows.into_iter().map(ExternalIdentity::from).collect())
        .map_err(db_error)
    }

    async fn touch_identity(
        &mut self,
        id: IdentityId,
        at: OffsetDateTime,
    ) -> Result<(), StorageError> {
        sqlx::query!(
            "update external_identities set last_used_at = $2 where id = $1",
            id.as_uuid(),
            at,
        )
        .execute(self.conn())
        .await
        .map_err(db_error)?;
        Ok(())
    }

    async fn delete_user_identity(
        &mut self,
        user: UserId,
        provider: &str,
    ) -> Result<bool, StorageError> {
        let result = sqlx::query!(
            "delete from external_identities where user_id = $1 and provider = $2",
            user.as_uuid(),
            provider,
        )
        .execute(self.conn())
        .await
        .map_err(db_error)?;
        Ok(result.rows_affected() > 0)
    }

    async fn delete_user_identities(&mut self, user: UserId) -> Result<u64, StorageError> {
        let result = sqlx::query!(
            "delete from external_identities where user_id = $1",
            user.as_uuid(),
        )
        .execute(self.conn())
        .await
        .map_err(db_error)?;
        Ok(result.rows_affected())
    }

    async fn create_oauth_flow(&mut self, flow: &OAuthFlow) -> Result<(), StorageError> {
        sqlx::query!(
            r#"
            insert into oauth_flows
                (state_hash, provider, pkce_verifier, nonce, link_user_id, redirect_to, expires_at)
            values ($1, $2, $3, $4, $5, $6, $7)
            "#,
            flow.state_hash.as_bytes().as_slice(),
            flow.provider,
            flow.pkce_verifier.expose(),
            flow.nonce,
            flow.link_user.map(|id| id.as_uuid()),
            flow.redirect_to,
            flow.expires_at,
        )
        .execute(self.conn())
        .await
        .map_err(db_error)?;
        Ok(())
    }

    async fn consume_oauth_flow(
        &mut self,
        state_hash: &TokenHash,
        now: OffsetDateTime,
    ) -> Result<Option<OAuthFlow>, StorageError> {
        let row = sqlx::query!(
            r#"
            delete from oauth_flows
            where state_hash = $1 and expires_at > $2
            returning state_hash, provider, pkce_verifier, nonce, link_user_id, redirect_to,
                expires_at
            "#,
            state_hash.as_bytes().as_slice(),
            now,
        )
        .fetch_optional(self.conn())
        .await
        .map_err(db_error)?;

        row.map(|row| {
            Ok(OAuthFlow {
                state_hash: TokenHash::from_slice(&row.state_hash).map_err(corrupt)?,
                provider: row.provider,
                pkce_verifier: Secret::new(row.pkce_verifier),
                nonce: row.nonce,
                link_user: row.link_user_id.map(UserId::from_uuid),
                redirect_to: row.redirect_to,
                expires_at: row.expires_at,
            })
        })
        .transpose()
    }

    async fn delete_expired_oauth_flows(
        &mut self,
        now: OffsetDateTime,
    ) -> Result<u64, StorageError> {
        let mut total = 0;
        loop {
            let deleted = sqlx::query!(
                r#"
                delete from oauth_flows
                where state_hash in (
                    select state_hash from oauth_flows where expires_at <= $1 limit $2
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
