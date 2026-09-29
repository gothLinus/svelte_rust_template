//! Conversions between the example resource's DTOs and its protobuf messages. Copy this file when
//! adding a resource (`just new-resource` does).
//!
//! DTO and message map field by field, by name:
//!
//! - `NoteDto` -> `v1::Note` ([`IntoMessage`]): ids become strings, timestamps become
//!   `google.protobuf.Timestamp`.
//! - `v1::CreateNoteRequest` and `v1::UpdateNoteRequest` -> the request DTOs ([`FromMessage`]):
//!   `optional` message fields are the DTO's `Option`s, so an absent field in a partial update
//!   keeps its value. A DTO with ids or enums parses them here and fails with the offending
//!   field's `ValidationErrors`; the value rules (lengths) are the service's.
//! - `page_message!` adds the paginated list, `v1::NotePage`.
//!
//! The rest of the slice: `proto/api/v1/notes.proto` (the messages), `domain::note`,
//! `application::notes` (DTOs, service, policy), `infrastructure::db::repositories::notes`,
//! `routes/notes.rs` (the handlers) and the migration that creates the table. Add the `mod` line
//! in `wire/mod.rs` and a `Permission` arm in `wire/common.rs` if the resource brings permissions.

use application::{
    ValidationErrors,
    notes::dto::{CreateNoteRequest, NoteDto, UpdateNoteRequest},
};
use proto::v1;

use super::{FromMessage, IntoMessage, page_message, timestamp};

impl IntoMessage for NoteDto {
    type Message = v1::Note;

    fn into_message(self) -> v1::Note {
        v1::Note {
            id: self.id.to_string(),
            owner_id: self.owner_id.to_string(),
            title: self.title,
            body: self.body,
            created_at: Some(timestamp(self.created_at)),
            updated_at: Some(timestamp(self.updated_at)),
        }
    }
}

page_message!(NoteDto => v1::NotePage);

impl FromMessage for CreateNoteRequest {
    type Message = v1::CreateNoteRequest;

    fn from_message(message: v1::CreateNoteRequest) -> Result<Self, ValidationErrors> {
        Ok(Self {
            title: message.title,
            body: message.body,
        })
    }
}

impl FromMessage for UpdateNoteRequest {
    type Message = v1::UpdateNoteRequest;

    fn from_message(message: v1::UpdateNoteRequest) -> Result<Self, ValidationErrors> {
        Ok(Self {
            title: message.title,
            body: message.body,
        })
    }
}
