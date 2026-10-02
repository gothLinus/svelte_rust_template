//! `/api/v1/notes`: the example resource, and the reference for handlers. Copy this file when
//! adding a resource (`just new-resource` does).
//!
//! A handler extracts, delegates to one service method and picks a status code:
//!
//! - [`CurrentUser`] rejects anonymous requests with `401`; `user.actor()` is what the service
//!   takes.
//! - Authorization, ownership included, is the service's job (`NoteService` with `NotePolicy`), so
//!   it holds for every caller of the service, not just this route. A refusal arrives as an
//!   `AppError` and `?` turns it into the problem response.
//! - Bodies are [`Proto`] DTOs both ways (see `crate::wire`); ids come from `Path<Uuid>` and are
//!   wrapped in the domain id type.
//! - Status codes: `200` for reads and updates, `201` with a `Location` for creation, `204` for
//!   deletion.
//! - Optimistic concurrency: responses with one note carry its version as [`ETag`]; update and
//!   delete take [`IfMatch`] and answer `412` if the note changed since (see
//!   `domain::repository::Version`).
//!
//! The per-IP API rate limit, CSRF check and session lookup apply to the whole `/api/v1` group
//! (see `crate::router`); an endpoint that needs a tighter limit calls `state.limits`, as
//! `routes/auth.rs` does.
//!
//! The rest of the slice: `domain::note`, `application::notes`,
//! `infrastructure::db::repositories::notes`, `wire/notes.rs`, `proto/api/v1/notes.proto` and the
//! migration that creates the table. Add the `mod` line and the `.merge` in `routes/mod.rs`.

use application::{
    Adapters,
    notes::dto::{CreateNoteRequest, ListNotesQuery, NoteDto, UpdateNoteRequest},
    pagination::PageDto,
};
use axum::{
    Router,
    extract::State,
    http::{HeaderValue, StatusCode, header::LOCATION},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use domain::{note::NoteId, repository::Version};
use uuid::Uuid;

use crate::{
    extract::{CurrentUser, ETag, IfMatch, Path, Proto, Query},
    problem::ApiError,
    state::AppState,
};

pub fn routes<A: Adapters>() -> Router<AppState<A>> {
    Router::new()
        .route("/notes", get(list::<A>).post(create::<A>))
        .route(
            "/notes/{id}",
            get(show::<A>).patch(update::<A>).delete(destroy::<A>),
        )
        .route("/notes/{id}/duplicate", post(duplicate::<A>))
}

async fn list<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
    Query(query): Query<ListNotesQuery>,
) -> Result<Proto<PageDto<NoteDto>>, ApiError> {
    Ok(Proto(state.services.notes.list(user.actor(), query).await?))
}

async fn show<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<(ETag, Proto<NoteDto>), ApiError> {
    let note = state
        .services
        .notes
        .get(user.actor(), NoteId::from_uuid(id))
        .await?;
    Ok(tagged(note))
}

async fn create<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
    Proto(body): Proto<CreateNoteRequest>,
) -> Result<Response, ApiError> {
    let note = state.services.notes.create(user.actor(), body).await?;
    created(note)
}

async fn duplicate<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<Response, ApiError> {
    let note = state
        .services
        .notes
        .duplicate(user.actor(), NoteId::from_uuid(id))
        .await?;
    created(note)
}

fn created(note: NoteDto) -> Result<Response, ApiError> {
    let location = HeaderValue::from_str(&format!("/api/v1/notes/{}", note.id))
        .map_err(|err| ApiError::internal(&err))?;
    Ok((StatusCode::CREATED, [(LOCATION, location)], tagged(note)).into_response())
}

fn tagged(note: NoteDto) -> (ETag, Proto<NoteDto>) {
    (ETag(Version::new(note.version)), Proto(note))
}

async fn update<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    IfMatch(expected): IfMatch,
    Proto(body): Proto<UpdateNoteRequest>,
) -> Result<(ETag, Proto<NoteDto>), ApiError> {
    let note = state
        .services
        .notes
        .update(user.actor(), NoteId::from_uuid(id), expected, body)
        .await?;
    Ok(tagged(note))
}

async fn destroy<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    IfMatch(expected): IfMatch,
) -> Result<StatusCode, ApiError> {
    state
        .services
        .notes
        .delete(user.actor(), NoteId::from_uuid(id), expected)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
