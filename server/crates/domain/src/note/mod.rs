//! Notes: the example resource, owned by one user, and the reference slice for adding your own.
//!
//! This module is the domain layer: value objects with validating constructors, the entity (with
//! a `Parts` struct for rebuilding it from storage), the create, update and filter types, and
//! the [`Resource`] impl that plugs them into the generic
//! [`Repository`](crate::repository::Repository). The rest of the slice:
//!
//! - `application::notes`: the service (a thin wrapper over `CrudService`), the DTOs and the
//!   policy (who may do what);
//! - `infrastructure::db::repositories::notes`: the Postgres `Repository<Note>`;
//! - `api`: `routes/notes.rs` (handlers) and `wire/notes.rs` (protobuf conversions), with the
//!   messages in `proto/api/v1/notes.proto`;
//! - `server/migrations/*_notes.up.sql`: the table and its index;
//! - `web/src/lib`: `api/notes.ts`, `components/note-*.svelte`, and the page under
//!   `routes/(app)/notes/`.
//!
//! `just new-resource <thing> <things>` copies the slice under new names and registers it
//! everywhere the example is registered. Left for you to change: the fields and their limits
//! (value objects, `NewNote`, `NoteChanges`, migration, queries, DTOs, proto messages) and the
//! policy's rules. The README's "How to add a new resource" lists every place.

use std::fmt::{self, Display, Formatter};

use time::OffsetDateTime;

use crate::{
    error::ValidationError,
    i18n::Message,
    id::Id,
    pagination::NewestFirst,
    repository::{Resource, Version, Versioned},
    user::UserId,
};

pub type NoteId = Id<Note>;

pub const MAX_NOTE_TITLE_LEN: usize = 200;
pub const MAX_NOTE_BODY_LEN: usize = 10_000;

/// A note's title: trimmed, 1 to [`MAX_NOTE_TITLE_LEN`] characters, one line, without control
/// characters or bidirectional overrides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteTitle(String);

impl NoteTitle {
    pub fn parse(raw: &str) -> Result<Self, ValidationError> {
        let title = raw.trim();
        let len = title.chars().count();

        if len == 0 {
            Err(ValidationError::required())
        } else if len > MAX_NOTE_TITLE_LEN {
            Err(ValidationError::new(
                "too_long",
                Message::new("note-title-too-long").arg("max", MAX_NOTE_TITLE_LEN),
            ))
        } else if title.chars().any(char::is_control) {
            Err(ValidationError::new(
                "invalid_characters",
                Message::new("note-title-single-line"),
            ))
        } else {
            Ok(Self(title.to_owned()))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for NoteTitle {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A note's text: up to [`MAX_NOTE_BODY_LEN`] characters. May be empty and may span lines; other
/// control characters are rejected.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NoteBody(String);

impl NoteBody {
    pub fn parse(raw: &str) -> Result<Self, ValidationError> {
        let body = raw.trim_end();

        if body.chars().count() > MAX_NOTE_BODY_LEN {
            Err(ValidationError::new(
                "too_long",
                Message::new("note-body-too-long").arg("max", MAX_NOTE_BODY_LEN),
            ))
        } else if body
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
        {
            Err(ValidationError::new(
                "invalid_characters",
                Message::new("note-body-control-characters"),
            ))
        } else {
            Ok(Self(body.to_owned()))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A note as stored. Its owner and holders of `notes:manage` may access it; the application
/// layer's policy enforces that, not this type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    id: NoteId,
    owner_id: UserId,
    title: NoteTitle,
    body: NoteBody,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
    version: Version,
}

pub struct NoteParts {
    pub id: NoteId,
    pub owner_id: UserId,
    pub title: NoteTitle,
    pub body: NoteBody,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub version: Version,
}

impl Note {
    pub fn from_parts(parts: NoteParts) -> Self {
        Self {
            id: parts.id,
            owner_id: parts.owner_id,
            title: parts.title,
            body: parts.body,
            created_at: parts.created_at,
            updated_at: parts.updated_at,
            version: parts.version,
        }
    }

    pub fn id(&self) -> NoteId {
        self.id
    }

    pub fn owner_id(&self) -> UserId {
        self.owner_id
    }

    pub fn title(&self) -> &NoteTitle {
        &self.title
    }

    pub fn body(&self) -> &NoteBody {
        &self.body
    }

    pub fn created_at(&self) -> OffsetDateTime {
        self.created_at
    }

    pub fn updated_at(&self) -> OffsetDateTime {
        self.updated_at
    }
}

/// Validated input for creating a note ([`Resource::Create`]). The id and the timestamps are
/// assigned when it is stored.
#[derive(Debug, Clone)]
pub struct NewNote {
    pub owner_id: UserId,
    pub title: NoteTitle,
    pub body: NoteBody,
}

#[derive(Debug, Clone, Default)]
pub struct NoteChanges {
    pub title: Option<NoteTitle>,
    pub body: Option<NoteBody>,
}

impl NoteChanges {
    pub fn is_empty(&self) -> bool {
        self.title.is_none() && self.body.is_none()
    }
}

/// Narrows a note list. `Default` lists every note, so services must always scope it through the
/// policy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct NoteFilter {
    pub owner_id: Option<UserId>,
}

impl Versioned for Note {
    fn version(&self) -> Version {
        self.version
    }
}

impl Resource for Note {
    type Id = NoteId;
    type Create = NewNote;
    type Update = NoteChanges;
    type Filter = NoteFilter;
    type Sort = NewestFirst;
}
