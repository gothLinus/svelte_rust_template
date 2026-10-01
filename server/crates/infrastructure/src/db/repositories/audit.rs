//! `AuditRepository`: the append-only `audit_events` table.

use std::net::IpAddr;

use domain::{
    audit::{AuditAction, AuditEvent, AuditEventId, AuditFilter, AuditRepository, NewAuditEvent},
    error::StorageError,
    pagination::{Cursor, Page, PageRequest},
    session::ClientInfo,
    user::UserId,
};
use time::OffsetDateTime;
use uuid::Uuid;

use super::{CLEANUP_BATCH, cleanup_done};
use crate::db::{
    errors::{corrupt, db_error, to_i64},
    postgres::{PgExecutor, PgHandle},
};

struct AuditEventRow {
    id: Uuid,
    user_id: Uuid,
    actor_id: Option<Uuid>,
    action: String,
    detail: Option<String>,
    ip: Option<IpAddr>,
    user_agent: Option<String>,
    occurred_at: OffsetDateTime,
}

impl TryFrom<AuditEventRow> for AuditEvent {
    type Error = StorageError;

    fn try_from(row: AuditEventRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: AuditEventId::from_uuid(row.id),
            user_id: UserId::from_uuid(row.user_id),
            actor_id: row.actor_id.map(UserId::from_uuid),
            action: AuditAction::parse(&row.action).map_err(corrupt)?,
            detail: row.detail,
            client: ClientInfo {
                ip: row.ip,
                user_agent: row.user_agent,
            },
            occurred_at: row.occurred_at,
        })
    }
}

impl<C: PgHandle> AuditRepository for PgExecutor<C> {
    async fn record_audit_event(&mut self, event: &NewAuditEvent) -> Result<(), StorageError> {
        sqlx::query!(
            r#"
            insert into audit_events
                (id, user_id, actor_id, action, detail, ip, user_agent, occurred_at)
            values ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
            event.id.as_uuid(),
            event.user_id.as_uuid(),
            event.actor_id.map(|actor| actor.as_uuid()),
            event.action.as_str(),
            event.detail.as_deref(),
            event.client.ip as Option<IpAddr>,
            event.client.user_agent.as_deref(),
            event.occurred_at,
        )
        .execute(self.conn())
        .await
        .map_err(db_error)?;
        Ok(())
    }

    /// Keyset pagination over the primary key, one static query per filter shape (see the notes
    /// repository for why).
    async fn list_audit_events(
        &mut self,
        filter: &AuditFilter,
        page: PageRequest,
    ) -> Result<Page<AuditEvent>, StorageError> {
        let after = page.after_id().unwrap_or(Uuid::max());
        let limit = to_i64(page.fetch_limit());
        let rows = match filter.user_id {
            Some(user) => {
                sqlx::query_as!(
                    AuditEventRow,
                    r#"
                    select id, user_id, actor_id, action, detail, ip as "ip: IpAddr", user_agent,
                        occurred_at
                    from audit_events
                    where user_id = $1 and id < $2
                    order by id desc
                    limit $3
                    "#,
                    user.as_uuid(),
                    after,
                    limit,
                )
                .fetch_all(self.conn())
                .await
            }
            None => {
                sqlx::query_as!(
                    AuditEventRow,
                    r#"
                    select id, user_id, actor_id, action, detail, ip as "ip: IpAddr", user_agent,
                        occurred_at
                    from audit_events
                    where id < $1
                    order by id desc
                    limit $2
                    "#,
                    after,
                    limit,
                )
                .fetch_all(self.conn())
                .await
            }
        }
        .map_err(db_error)?
        .into_iter()
        .map(AuditEvent::try_from)
        .collect::<Result<Vec<_>, _>>()?;

        Ok(Page::from_rows(rows, &page, |event| {
            Cursor::from_uuid(event.id.as_uuid())
        }))
    }

    async fn delete_audit_events_before(
        &mut self,
        cutoff: OffsetDateTime,
    ) -> Result<u64, StorageError> {
        let mut total = 0;
        loop {
            let deleted = sqlx::query!(
                r#"
                delete from audit_events
                where id in (
                    select id from audit_events where occurred_at < $1 limit $2
                )
                "#,
                cutoff,
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
