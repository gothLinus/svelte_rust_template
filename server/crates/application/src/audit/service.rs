use std::{collections::HashMap, sync::Arc};

use domain::{
    audit::{AuditFilter, AuditRepository},
    database::Database,
    pagination::PageRequest,
    rbac::Permission,
    user::{User, UserId, UserRepository},
};

use crate::{
    Adapters, Context,
    actor::Actor,
    audit::dto::{AuditEventDto, AuditUserDto, ListActivityQuery, ListAuditQuery},
    error::AppError,
    pagination::PageDto,
};

pub struct AuditService<A: Adapters> {
    ctx: Arc<Context<A>>,
}

impl<A: Adapters> AuditService<A> {
    pub fn new(ctx: Arc<Context<A>>) -> Self {
        Self { ctx }
    }

    /// The actor's own events, newest first. Who else caused one stays unnamed: the owner learns
    /// that an administrator acted, not which one.
    pub async fn activity(
        &self,
        actor: &Actor,
        query: ListActivityQuery,
    ) -> Result<PageDto<AuditEventDto>, AppError> {
        let page = PageRequest::try_from(query)?;
        let filter = AuditFilter {
            user_id: Some(actor.user_id),
        };
        let events = self
            .ctx
            .db
            .connection()
            .await?
            .list_audit_events(&filter, page)
            .await?;
        Ok(events.map(|event| AuditEventDto::new(&event)).into())
    }

    /// Every account's events, or one account's, with the accounts named.
    ///
    /// # Errors
    ///
    /// `Forbidden` without `audit:read`.
    pub async fn list(
        &self,
        actor: &Actor,
        query: ListAuditQuery,
    ) -> Result<PageDto<AuditEventDto>, AppError> {
        actor.require(Permission::AuditRead)?;
        let (filter, page) = query.try_into()?;

        let mut conn = self.ctx.db.connection().await?;
        let events = conn.list_audit_events(&filter, page).await?;
        let mut ids: Vec<UserId> = events
            .items
            .iter()
            .flat_map(|event| [Some(event.user_id), event.actor_id])
            .flatten()
            .collect();
        ids.sort_unstable();
        ids.dedup();
        let users: HashMap<UserId, AuditUserDto> = conn
            .find_users(&ids)
            .await?
            .iter()
            .map(|user: &User| (user.id(), AuditUserDto::from(user)))
            .collect();

        Ok(events
            .map(|event| AuditEventDto {
                user: users.get(&event.user_id).cloned(),
                actor: event
                    .actor_id
                    .filter(|actor| *actor != event.user_id)
                    .and_then(|actor| users.get(&actor).cloned()),
                ..AuditEventDto::new(&event)
            })
            .into())
    }
}
