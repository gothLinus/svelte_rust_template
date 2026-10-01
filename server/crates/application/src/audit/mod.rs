//! The audit log: what happened to an account's security, read back by its owner (`/me/activity`)
//! and by holders of `audit:read` (`/admin/audit`).
//!
//! Events are written by the use cases that cause them, in their own transaction, through
//! [`Context::event`](crate::Context) and [`Context::actor_event`](crate::Context); this module only
//! reads them. [`AuditService::delete_expired`] is not here: retention runs with the other cleanup in
//! [`crate::maintenance`].

pub use service::AuditService;

pub mod dto;

mod service;
