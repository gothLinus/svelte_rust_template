//! Notes, the example resource: a user-owned entity with list, get, create, update and delete,
//! plus one use case beyond CRUD (`NoteService::duplicate`). The README's "How to add a new
//! resource" walks through copying it; `just new-resource <thing> <things>` does the copying and
//! renaming in every layer.
//!
//! The pieces of the slice:
//!
//! - `domain::note`: the entity, its validating value objects, `NewNote`, `NoteChanges`,
//!   `NoteFilter` and the [`Resource`](domain::repository::Resource) impl tying them together.
//! - This module: `NoteService` (wraps [`CrudService`](crate::crud::CrudService)), [`dto`],
//!   `NotePolicy` and the [`Exportable`](crate::export::Exportable) impl for the personal data
//!   export.
//! - `infrastructure::db::repositories::notes` and the `..._notes` migration: storage, as a
//!   Postgres `Repository<Note>`.
//! - `api::routes::notes` and `api::wire::notes`: the handlers and the conversions to and from
//!   `proto/api/v1/notes.proto`; the frontend lives in `web/src/lib/api/notes.ts` and
//!   `web/src/routes/(app)/notes`.
//!
//! The scaffolder also registers the resource in [`Store`](crate::Store),
//! [`Services`](crate::Services), the permission list, [`crate::types::limits`] and
//! [`EXPORTED_TABLES`](crate::export::EXPORTED_TABLES). What it leaves for you to edit: the fields
//! (`title`, `body`) and their limits, the rules in `NotePolicy`, and the JSON shape in the
//! `Exportable` impl below.

pub use policy::NotePolicy;
pub use service::NoteService;

pub mod dto;

mod policy;
mod service;

impl crate::export::Exportable for domain::note::Note {
    const SECTION: &'static str = "notes";
    const TABLE: &'static str = "notes";

    fn owned_by(user: domain::user::UserId) -> domain::note::NoteFilter {
        domain::note::NoteFilter {
            owner_id: Some(user),
        }
    }

    fn export(&self) -> serde_json::Value {
        use crate::export::timestamp;
        serde_json::json!({
            "id": self.id().to_string(),
            "title": self.title().as_str(),
            "body": self.body().as_str(),
            "created_at": timestamp(self.created_at()),
            "updated_at": timestamp(self.updated_at()),
        })
    }
}
