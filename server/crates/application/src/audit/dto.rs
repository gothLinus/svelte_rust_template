use domain::{
    audit::{AuditEvent, AuditFilter},
    pagination::{NewestFirst, PageRequest},
    user::{User, UserId},
};
use serde::Deserialize;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{error::ValidationErrors, pagination::page_request};

/// Query string of a user's own activity: `?limit=20&after=<cursor>`.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListActivityQuery {
    pub limit: Option<u32>,
    pub after: Option<String>,
}

impl TryFrom<ListActivityQuery> for PageRequest {
    type Error = ValidationErrors;

    fn try_from(query: ListActivityQuery) -> Result<Self, Self::Error> {
        page_request(query.limit, query.after.as_deref(), NewestFirst)
    }
}

/// Query string of the audit log: `?user=<id>&limit=20&after=<cursor>`; without `user`, every
/// account's events.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListAuditQuery {
    pub user: Option<String>,
    pub limit: Option<u32>,
    pub after: Option<String>,
}

impl TryFrom<ListAuditQuery> for (AuditFilter, PageRequest) {
    type Error = ValidationErrors;

    fn try_from(query: ListAuditQuery) -> Result<Self, Self::Error> {
        let page = page_request(query.limit, query.after.as_deref(), NewestFirst);
        let mut errors = page.as_ref().err().cloned().unwrap_or_default();
        let user = match query.user.as_deref().filter(|raw| !raw.trim().is_empty()) {
            Some(raw) => errors.check("user", UserId::parse(raw)),
            None => None,
        };
        match page {
            Ok(page) if errors.is_empty() => Ok((AuditFilter { user_id: user }, page)),
            _ => Err(errors),
        }
    }
}

/// An account named in the audit log.
#[derive(Debug, Clone)]
pub struct AuditUserDto {
    pub id: Uuid,
    pub username: String,
    pub email: String,
}

impl From<&User> for AuditUserDto {
    fn from(user: &User) -> Self {
        Self {
            id: user.id().as_uuid(),
            username: user.username().to_string(),
            email: user.email().to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AuditEventDto {
    pub id: Uuid,
    /// The stable name of a `domain::audit::AuditAction`, such as `signed_in`.
    pub action: String,
    pub detail: Option<String>,
    /// The account it is about. Only in the audit log; a user's own activity leaves it out.
    pub user: Option<AuditUserDto>,
    /// Who caused it, when that was someone else whose account still exists. Only in the audit
    /// log.
    pub actor: Option<AuditUserDto>,
    /// Someone other than the account caused it (an administrator).
    pub by_other: bool,
    pub ip: Option<String>,
    pub user_agent: Option<String>,
    pub occurred_at: OffsetDateTime,
}

impl AuditEventDto {
    pub(crate) fn new(event: &AuditEvent) -> Self {
        Self {
            id: event.id.as_uuid(),
            action: event.action.as_str().to_owned(),
            detail: event.detail.clone(),
            user: None,
            actor: None,
            by_other: event.actor_id.is_some_and(|actor| actor != event.user_id),
            ip: event.client.ip.map(|ip| ip.to_string()),
            user_agent: event.client.user_agent.clone(),
            occurred_at: event.occurred_at,
        }
    }
}
