//! Files: a user-owned resource with contents. The rows go through the generic
//! [`CrudService`](crate::crud::CrudService) like the notes'; the contents are uploaded to and
//! downloaded from the [`ObjectStore`](domain::object_store::ObjectStore) by use cases of their
//! own (`FileService::upload`, `FileService::download`).
//!
//! The pieces of the slice:
//!
//! - `domain::file`: the entity and its value objects, and `domain::object_store`: the store and
//!   the queue of objects to remove.
//! - This module: `FileService`, [`dto`], `FilePolicy` and the
//!   [`Exportable`](crate::export::Exportable) impl for the personal data export.
//! - `infrastructure::db::repositories::files` and `object_deletions`, the `..._files` migration,
//!   and `infrastructure::object_store` (S3, which RustFS speaks).
//! - `api::routes::files` and `api::wire::files`, with `proto/api/v1/files.proto`; the frontend in
//!   `web/src/lib/api/files.ts` and `web/src/routes/(app)/files`.
//!
//! Contents are removed from the store after the rows that name them, never inside a transaction:
//! see `domain::object_store`, and [`crate::maintenance`], which empties the queue.

pub use policy::FilePolicy;
pub use service::{FileService, UNFINISHED_UPLOAD_TTL};

pub mod dto;

mod policy;
mod service;

pub(crate) use service::purge_object;

impl crate::export::Exportable for domain::file::StoredFile {
    const SECTION: &'static str = "files";
    const TABLE: &'static str = "files";

    fn owned_by(user: domain::user::UserId) -> domain::file::FileFilter {
        domain::file::FileFilter {
            owner_id: Some(user),
        }
    }

    fn export(&self) -> serde_json::Value {
        use crate::export::timestamp;
        serde_json::json!({
            "id": self.id().to_string(),
            "name": self.name().as_str(),
            "content_type": self.content_type().as_str(),
            "size": self.size().bytes(),
            "created_at": timestamp(self.created_at()),
            "updated_at": timestamp(self.updated_at()),
        })
    }
}
