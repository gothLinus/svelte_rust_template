use std::sync::Arc;

use domain::{
    database::{Database, Transaction},
    note::{Note, NoteId},
};

use crate::{
    Adapters, Context,
    actor::Actor,
    crud::CrudService,
    error::AppError,
    notes::{
        NotePolicy,
        dto::{CreateNoteRequest, ListNotesQuery, NoteDto, UpdateNoteRequest},
    },
    pagination::PageDto,
    policy::Action,
};

/// Notes CRUD. Everything is delegated to the generic [`CrudService`]; this wrapper only fixes the
/// types and turns query parameters into a filter. Use cases beyond CRUD (sharing, archiving)
/// go here too, like [`NoteService::duplicate`]: authorize through the `CrudService`, then do the
/// work, in one transaction when it writes.
///
/// Each method takes the [`Actor`] of the request and returns a response DTO; the failures are
/// those of [`CrudService`] (`NotFound`, `Forbidden`, `Validation`).
pub struct NoteService<A: Adapters> {
    crud: CrudService<A, Note, NotePolicy>,
}

impl<A: Adapters> NoteService<A> {
    pub fn new(ctx: Arc<Context<A>>) -> Self {
        Self {
            crud: CrudService::new(ctx),
        }
    }

    pub async fn list(
        &self,
        actor: &Actor,
        query: ListNotesQuery,
    ) -> Result<PageDto<NoteDto>, AppError> {
        let (filter, page) = query.into_parts(actor)?;
        let page = self.crud.list(actor, filter, page).await?;
        Ok(page.into())
    }

    pub async fn get(&self, actor: &Actor, id: NoteId) -> Result<NoteDto, AppError> {
        self.crud.get(actor, id).await
    }

    pub async fn create(
        &self,
        actor: &Actor,
        request: CreateNoteRequest,
    ) -> Result<NoteDto, AppError> {
        self.crud.create(actor, request).await
    }

    pub async fn update(
        &self,
        actor: &Actor,
        id: NoteId,
        request: UpdateNoteRequest,
    ) -> Result<NoteDto, AppError> {
        self.crud.update(actor, id, request).await
    }

    pub async fn delete(&self, actor: &Actor, id: NoteId) -> Result<(), AppError> {
        self.crud.delete(actor, id).await
    }

    /// A copy of a note the actor can see, owned by the actor. An example of a use case beyond
    /// CRUD: the source is authorized like any read (one the actor may not see stays a `NotFound`),
    /// the copy like any create, and both happen in one transaction.
    pub async fn duplicate(&self, actor: &Actor, id: NoteId) -> Result<NoteDto, AppError> {
        let mut tx = self.crud.context().db.transaction().await?;
        let source = self
            .crud
            .authorize_in(&mut tx, actor, id, Action::Read)
            .await?;
        let copy = self
            .crud
            .create_in(&mut tx, actor, CreateNoteRequest::copy_of(&source))
            .await?;
        tx.commit().await?;
        Ok(copy)
    }
}
