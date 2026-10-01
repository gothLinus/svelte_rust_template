//! `SessionRepository`: server-side sessions keyed by the digest of an opaque token.
//!
//! `find_session_by_token` returns the session, its user and the user's effective permissions in
//! one query, so authenticating a request costs a single round trip. `rotate_session` only
//! succeeds while the stored digest still equals the previous one, which is what makes concurrent
//! rotations safe.

use std::net::IpAddr;

use domain::{
    error::StorageError,
    secret::TokenHash,
    session::{
        AuthenticatedSession, ClientInfo, Session, SessionId, SessionParts, SessionRepository,
    },
    user::{User, UserId},
};
use time::OffsetDateTime;
use uuid::Uuid;

use super::{CLEANUP_BATCH, cleanup_done};
use crate::db::{
    errors::{corrupt, db_error},
    postgres::{PgExecutor, PgHandle},
    repositories::{rbac::permission_set, users::UserRow},
};

struct SessionRow {
    id: Uuid,
    user_id: Uuid,
    token_hash: Vec<u8>,
    ip: Option<IpAddr>,
    user_agent: Option<String>,
    rotation_pending: bool,
    created_at: OffsetDateTime,
    last_seen_at: OffsetDateTime,
    expires_at: OffsetDateTime,
    reauthenticated_at: OffsetDateTime,
}

impl TryFrom<SessionRow> for Session {
    type Error = StorageError;

    fn try_from(row: SessionRow) -> Result<Self, Self::Error> {
        Ok(Self::from_parts(SessionParts {
            id: SessionId::from_uuid(row.id),
            user_id: UserId::from_uuid(row.user_id),
            token_hash: TokenHash::from_slice(&row.token_hash).map_err(corrupt)?,
            client: ClientInfo {
                ip: row.ip,
                user_agent: row.user_agent,
            },
            created_at: row.created_at,
            last_seen_at: row.last_seen_at,
            expires_at: row.expires_at,
            rotation_pending: row.rotation_pending,
            reauthenticated_at: row.reauthenticated_at,
        }))
    }
}

struct AuthenticatedRow {
    id: Uuid,
    user_id: Uuid,
    token_hash: Vec<u8>,
    ip: Option<IpAddr>,
    user_agent: Option<String>,
    rotation_pending: bool,
    created_at: OffsetDateTime,
    last_seen_at: OffsetDateTime,
    expires_at: OffsetDateTime,
    reauthenticated_at: OffsetDateTime,
    email: String,
    username: String,
    phone: Option<String>,
    phone_verified_at: Option<OffsetDateTime>,
    password_hash: Option<String>,
    email_verified_at: Option<OffsetDateTime>,
    disabled_at: Option<OffsetDateTime>,
    locale: Option<String>,
    user_created_at: OffsetDateTime,
    user_updated_at: OffsetDateTime,
    permissions: Vec<String>,
}

impl TryFrom<AuthenticatedRow> for AuthenticatedSession {
    type Error = StorageError;

    fn try_from(row: AuthenticatedRow) -> Result<Self, Self::Error> {
        let user = User::try_from(UserRow {
            id: row.user_id,
            email: row.email,
            username: row.username,
            phone: row.phone,
            phone_verified_at: row.phone_verified_at,
            password_hash: row.password_hash,
            email_verified_at: row.email_verified_at,
            disabled_at: row.disabled_at,
            locale: row.locale,
            created_at: row.user_created_at,
            updated_at: row.user_updated_at,
        })?;
        let session = Session::try_from(SessionRow {
            id: row.id,
            user_id: row.user_id,
            token_hash: row.token_hash,
            ip: row.ip,
            user_agent: row.user_agent,
            rotation_pending: row.rotation_pending,
            created_at: row.created_at,
            last_seen_at: row.last_seen_at,
            expires_at: row.expires_at,
            reauthenticated_at: row.reauthenticated_at,
        })?;

        Ok(Self {
            session,
            user,
            permissions: permission_set(&row.permissions),
        })
    }
}

impl<C: PgHandle> SessionRepository for PgExecutor<C> {
    async fn create_session(&mut self, session: &Session) -> Result<(), StorageError> {
        sqlx::query!(
            r#"
            insert into sessions
                (id, user_id, token_hash, ip, user_agent, created_at, last_seen_at, expires_at,
                 reauthenticated_at)
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            "#,
            session.id().as_uuid(),
            session.user_id().as_uuid(),
            session.token_hash().as_bytes().as_slice(),
            session.client().ip as Option<IpAddr>,
            session.client().user_agent.as_deref(),
            session.created_at(),
            session.last_seen_at(),
            session.expires_at(),
            session.reauthenticated_at(),
        )
        .execute(self.conn())
        .await
        .map_err(db_error)?;

        Ok(())
    }

    async fn find_session_by_token(
        &mut self,
        token_hash: &TokenHash,
    ) -> Result<Option<AuthenticatedSession>, StorageError> {
        // One round trip per authenticated request: the session, its user and the user's effective
        // permissions.
        sqlx::query_as!(
            AuthenticatedRow,
            r#"
            select
                s.id, s.user_id, s.token_hash, s.ip as "ip: IpAddr", s.user_agent,
                s.rotation_pending, s.created_at, s.last_seen_at, s.expires_at,
                s.reauthenticated_at,
                u.email, u.username, u.phone, u.phone_verified_at,
                u.password_hash, u.email_verified_at, u.disabled_at, u.locale,
                u.created_at as user_created_at, u.updated_at as user_updated_at,
                array(
                    select distinct rp.permission
                    from user_roles ur
                    join role_permissions rp on rp.role = ur.role
                    where ur.user_id = u.id
                ) as "permissions!"
            from sessions s
            join users u on u.id = s.user_id
            where s.token_hash = $1
            "#,
            token_hash.as_bytes().as_slice(),
        )
        .fetch_optional(self.conn())
        .await
        .map_err(db_error)?
        .map(AuthenticatedSession::try_from)
        .transpose()
    }

    async fn touch_session(&mut self, session: &Session) -> Result<bool, StorageError> {
        let result = sqlx::query!(
            "update sessions set last_seen_at = $2 where id = $1",
            session.id().as_uuid(),
            session.last_seen_at(),
        )
        .execute(self.conn())
        .await
        .map_err(db_error)?;

        Ok(result.rows_affected() > 0)
    }

    async fn rotate_session(
        &mut self,
        session: &Session,
        previous: &TokenHash,
    ) -> Result<bool, StorageError> {
        let result = sqlx::query!(
            r#"
            update sessions
            set token_hash = $2, last_seen_at = $3, rotation_pending = false
            where id = $1 and token_hash = $4
            "#,
            session.id().as_uuid(),
            session.token_hash().as_bytes().as_slice(),
            session.last_seen_at(),
            previous.as_bytes().as_slice(),
        )
        .execute(self.conn())
        .await
        .map_err(db_error)?;

        Ok(result.rows_affected() > 0)
    }

    async fn mark_session_reauthenticated(
        &mut self,
        id: SessionId,
        at: OffsetDateTime,
    ) -> Result<bool, StorageError> {
        let result = sqlx::query!(
            "update sessions set reauthenticated_at = $2 where id = $1",
            id.as_uuid(),
            at,
        )
        .execute(self.conn())
        .await
        .map_err(db_error)?;

        Ok(result.rows_affected() > 0)
    }

    async fn flag_user_sessions_for_rotation(&mut self, user: UserId) -> Result<u64, StorageError> {
        let result = sqlx::query!(
            "update sessions set rotation_pending = true where user_id = $1",
            user.as_uuid(),
        )
        .execute(self.conn())
        .await
        .map_err(db_error)?;

        Ok(result.rows_affected())
    }

    async fn delete_session(&mut self, id: SessionId) -> Result<bool, StorageError> {
        let result = sqlx::query!("delete from sessions where id = $1", id.as_uuid())
            .execute(self.conn())
            .await
            .map_err(db_error)?;

        Ok(result.rows_affected() > 0)
    }

    async fn delete_session_by_token(
        &mut self,
        token_hash: &TokenHash,
    ) -> Result<bool, StorageError> {
        let result = sqlx::query!(
            "delete from sessions where token_hash = $1",
            token_hash.as_bytes().as_slice(),
        )
        .execute(self.conn())
        .await
        .map_err(db_error)?;

        Ok(result.rows_affected() > 0)
    }

    async fn delete_user_session(
        &mut self,
        user: UserId,
        id: SessionId,
    ) -> Result<bool, StorageError> {
        let result = sqlx::query!(
            "delete from sessions where id = $1 and user_id = $2",
            id.as_uuid(),
            user.as_uuid(),
        )
        .execute(self.conn())
        .await
        .map_err(db_error)?;

        Ok(result.rows_affected() > 0)
    }

    async fn delete_user_sessions(
        &mut self,
        user: UserId,
        keep: Option<SessionId>,
    ) -> Result<u64, StorageError> {
        let result = sqlx::query!(
            "delete from sessions where user_id = $1 and id is distinct from $2",
            user.as_uuid(),
            keep.map(|id| id.as_uuid()),
        )
        .execute(self.conn())
        .await
        .map_err(db_error)?;

        Ok(result.rows_affected())
    }

    async fn list_user_sessions(&mut self, user: UserId) -> Result<Vec<Session>, StorageError> {
        sqlx::query_as!(
            SessionRow,
            r#"
            select id, user_id, token_hash, ip as "ip: IpAddr", user_agent, rotation_pending,
                created_at, last_seen_at, expires_at, reauthenticated_at
            from sessions
            where user_id = $1
            order by last_seen_at desc, id desc
            "#,
            user.as_uuid(),
        )
        .fetch_all(self.conn())
        .await
        .map_err(db_error)?
        .into_iter()
        .map(Session::try_from)
        .collect()
    }

    async fn delete_expired_sessions(
        &mut self,
        now: OffsetDateTime,
        idle_cutoff: OffsetDateTime,
    ) -> Result<u64, StorageError> {
        let mut total = 0;
        loop {
            let deleted = sqlx::query!(
                r#"
                delete from sessions
                where id in (
                    select id from sessions
                    where expires_at <= $1 or last_seen_at <= $2
                    limit $3
                )
                and (expires_at <= $1 or last_seen_at <= $2)
                "#,
                now,
                idle_cutoff,
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
