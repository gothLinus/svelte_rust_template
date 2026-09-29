//! Request and response bodies of the notes endpoints, and their validation. Requests hold raw
//! strings and are validated through the domain's value objects when converted; the wire layer
//! maps each DTO to a message of `proto/api/v1/notes.proto`.

use domain::{
    note::{NewNote, Note, NoteBody, NoteChanges, NoteFilter, NoteTitle},
    pagination::{NewestFirst, PageRequest},
};
use serde::Deserialize;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    actor::Actor,
    crud::{Changes, Input},
    error::ValidationErrors,
    pagination::page_request,
};

#[derive(Debug, Clone)]
pub struct NoteDto {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub title: String,
    pub body: String,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

impl From<Note> for NoteDto {
    fn from(note: Note) -> Self {
        Self {
            id: note.id().as_uuid(),
            owner_id: note.owner_id().as_uuid(),
            title: note.title().to_string(),
            body: note.body().as_str().to_owned(),
            created_at: note.created_at(),
            updated_at: note.updated_at(),
        }
    }
}

#[derive(Debug)]
pub struct CreateNoteRequest {
    pub title: String,
    pub body: Option<String>,
}

impl CreateNoteRequest {
    pub fn copy_of(note: &Note) -> Self {
        Self {
            title: note.title().to_string(),
            body: Some(note.body().as_str().to_owned()),
        }
    }
}

impl Input<NewNote> for CreateNoteRequest {
    fn into_domain(self, actor: &Actor) -> Result<NewNote, ValidationErrors> {
        let mut errors = ValidationErrors::new();
        let title = errors.check("title", NoteTitle::parse(&self.title));
        let body = errors.check("body", NoteBody::parse(self.body.as_deref().unwrap_or("")));

        match (title, body) {
            (Some(title), Some(body)) => Ok(NewNote {
                owner_id: actor.user_id,
                title,
                body,
            }),
            _ => Err(errors),
        }
    }
}

/// Body of the update endpoint, a partial update: omitted fields keep their value. Converted to
/// `NoteChanges`.
#[derive(Debug)]
pub struct UpdateNoteRequest {
    pub title: Option<String>,
    pub body: Option<String>,
}

impl Changes<Note> for UpdateNoteRequest {
    fn into_changes(
        self,
        _actor: &Actor,
        _current: &Note,
    ) -> Result<NoteChanges, ValidationErrors> {
        self.try_into()
    }
}

impl TryFrom<UpdateNoteRequest> for NoteChanges {
    type Error = ValidationErrors;

    fn try_from(request: UpdateNoteRequest) -> Result<Self, Self::Error> {
        let mut errors = ValidationErrors::new();
        let title = request
            .title
            .map(|title| errors.check("title", NoteTitle::parse(&title)));
        let body = request
            .body
            .map(|body| errors.check("body", NoteBody::parse(&body)));

        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(Self {
            title: title.flatten(),
            body: body.flatten(),
        })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NoteScope {
    #[default]
    Mine,
    All,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListNotesQuery {
    pub scope: Option<NoteScope>,
    pub limit: Option<u32>,
    pub after: Option<String>,
}

impl ListNotesQuery {
    /// The filter as requested and the validated page; the policy then checks the filter (asking
    /// for `All` without `notes:manage` is `Forbidden`).
    pub fn into_parts(self, actor: &Actor) -> Result<(NoteFilter, PageRequest), ValidationErrors> {
        let page = page_request(self.limit, self.after.as_deref(), NewestFirst)?;
        let filter = match self.scope.unwrap_or_default() {
            NoteScope::Mine => NoteFilter {
                owner_id: Some(actor.user_id),
            },
            NoteScope::All => NoteFilter { owner_id: None },
        };
        Ok((filter, page))
    }
}
