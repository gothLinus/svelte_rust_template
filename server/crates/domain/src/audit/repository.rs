use std::future::Future;

use time::OffsetDateTime;

use crate::{
    audit::{AuditEvent, AuditFilter, NewAuditEvent},
    error::StorageError,
    pagination::{Page, PageRequest},
};

/// Storage for the audit log. Events are only ever appended, listed and deleted by age.
#[diagnostic::on_unimplemented(
    message = "`{Self}` does not implement `AuditRepository`",
    note = "every port a service uses must be part of `application::Store` (context.rs: the trait and its blanket impl) and implemented for `PgExecutor<C>` in infrastructure/src/db/repositories/"
)]
pub trait AuditRepository: Send {
    fn record_audit_event(
        &mut self,
        event: &NewAuditEvent,
    ) -> impl Future<Output = Result<(), StorageError>> + Send;

    /// Newest first.
    fn list_audit_events(
        &mut self,
        filter: &AuditFilter,
        page: PageRequest,
    ) -> impl Future<Output = Result<Page<AuditEvent>, StorageError>> + Send;

    /// Retention: deletes events that occurred before `cutoff`, and returns how many.
    fn delete_audit_events_before(
        &mut self,
        cutoff: OffsetDateTime,
    ) -> impl Future<Output = Result<u64, StorageError>> + Send;
}
