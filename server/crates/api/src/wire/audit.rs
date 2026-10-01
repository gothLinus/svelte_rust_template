use application::audit::dto::{AuditEventDto, AuditUserDto};
use proto::v1;

use super::{IntoMessage, page_message, timestamp};

impl IntoMessage for AuditUserDto {
    type Message = v1::AuditUser;

    fn into_message(self) -> v1::AuditUser {
        v1::AuditUser {
            id: self.id.to_string(),
            username: self.username,
            email: self.email,
        }
    }
}

impl IntoMessage for AuditEventDto {
    type Message = v1::AuditEvent;

    fn into_message(self) -> v1::AuditEvent {
        v1::AuditEvent {
            id: self.id.to_string(),
            action: self.action,
            detail: self.detail,
            user: self.user.map(IntoMessage::into_message),
            actor: self.actor.map(IntoMessage::into_message),
            by_other: self.by_other,
            ip: self.ip,
            user_agent: self.user_agent,
            occurred_at: Some(timestamp(self.occurred_at)),
        }
    }
}

page_message!(AuditEventDto => v1::AuditEventPage);
