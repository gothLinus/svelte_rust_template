//! The generic create/read/update/delete flow, reused by every CRUD resource.
//!
//! Each operation runs the same steps: **authorize → validate → persist → present**. A resource
//! plugs in its [`Policy`] and its request DTOs; the concrete service (such as `NoteService`)
//! wraps a `CrudService` and adds whatever is specific to it.
//!
//! Every operation comes twice:
//!
//! - `create`, `update`, ... get their own connection (or, for writes that read first, their own
//!   transaction) and are what a service usually calls;
//! - `create_in`, `update_in`, ... run on a store the caller passes: a transaction that also
//!   writes something else, which then commits or rolls back as one.
//!
//! A use case beyond CRUD (archive, share, toggle) starts with [`CrudService::authorize`] or
//! [`CrudService::authorize_in`], which load the entity and run the policy exactly like the CRUD
//! operations do, so a copied use case cannot forget the check.
//!
//! If a call does not compile with "the method exists but its trait bounds were not satisfied",
//! the entity is missing from [`Store`] or its adapter has no `Repository<E>` impl.

use std::{marker::PhantomData, sync::Arc};

use domain::{
    clock::Clock,
    database::{Database, Transaction},
    id::NewId,
    pagination::{Page, PageRequest},
    repository::{Repository, Resource, Version, Versioned},
};

use crate::{
    Adapters, Context, Store,
    actor::Actor,
    error::{AppError, ValidationErrors},
    policy::{Action, Policy},
};

/// Turns a create request into what the repository stores. Unlike `TryFrom`, it sees the actor,
/// so it can fill in the owner.
///
/// Implemented by the request DTO; `T` is the entity's [`Resource::Create`]. Validate every field
/// and return all failures together, not just the first.
pub trait Input<T> {
    fn into_domain(self, actor: &Actor) -> Result<T, ValidationErrors>;
}

/// Turns an update request into the entity's changes. It sees the actor and the entity as it is
/// now, for `updated_by`, fields only some actors may set, or rules about which state may follow
/// which. A request that needs neither delegates to its `TryFrom`, like `UpdateNoteRequest`.
/// There is no blanket impl for `TryInto` types: it would forbid writing this impl by hand for
/// any other request.
///
/// Implemented by the request DTO; the result is the entity's [`Resource::Update`].
pub trait Changes<E: Resource> {
    fn into_changes(self, actor: &Actor, current: &E) -> Result<E::Update, ValidationErrors>;
}

/// Turns an entity into what the actor gets back, such as with a `canEdit` flag that depends on
/// who is asking. Every `From<E>` is one that does not care.
///
/// Implemented by the response DTO for the entity `E`.
pub trait Present<E>: Sized {
    fn present(entity: E, actor: &Actor) -> Self;
}

impl<E, T: From<E>> Present<E> for T {
    fn present(entity: E, _actor: &Actor) -> Self {
        Self::from(entity)
    }
}

/// CRUD for resource `E` under policy `P`.
///
/// - `A`: the [`Adapters`] the application runs on; supplies the database and the clock.
/// - `E`: the [`Resource`] (entity) being managed. It fixes the id, create, update, filter
///   and sort types; its `Repository<E>` must be part of [`Store`].
/// - `P`: the [`Policy<E>`](Policy) that decides who may do what.
///
/// Request and response DTOs are not type parameters: each method takes them as generics bounded by
/// [`Input`], [`Changes`] and [`Present`], so one service can serve several shapes of the same
/// entity.
///
/// # Errors
///
/// Every method returns [`AppError`]:
///
/// - `NotFound` if the entity does not exist or the actor may not even read it, so ids of
///   other users' resources cannot be probed. This holds for updates and deletes too.
/// - `Forbidden` if the actor may read the entity but not do the action, may not create, or
///   may not list the requested slice.
/// - `Validation` if the request DTO's conversion fails; checked after authorization.
/// - `Internal` for storage failures.
///
/// # Examples
///
/// A concrete service fixes the types and forwards:
///
/// ```
/// use std::sync::Arc;
///
/// use application::{
///     Adapters, AppError, Context,
///     actor::Actor,
///     crud::CrudService,
///     notes::dto::{CreateNoteRequest, NoteDto},
///     policy::{Action, SimplePolicy, owner_or},
/// };
/// use domain::{
///     note::{Note, NoteFilter, NoteId},
///     rbac::Permission::{NotesManage, NotesRead, NotesWrite},
/// };
///
/// /// Everyone reads and writes their own notes; `notes:manage` covers all.
/// struct OwnNotes;
///
/// impl SimplePolicy<Note> for OwnNotes {
///     fn can(actor: &Actor, action: Action, note: Option<&Note>) -> bool {
///         match (action, note) {
///             (Action::Read, None) => actor.has(NotesRead),
///             (Action::Create, _) => actor.has(NotesWrite),
///             (Action::Read, Some(note)) => owner_or(actor, note, NotesRead, NotesManage),
///             (Action::Update | Action::Delete, Some(note)) => {
///                 owner_or(actor, note, NotesWrite, NotesManage)
///             }
///             (Action::Update | Action::Delete, None) => false,
///         }
///     }
///
///     fn scope(actor: &Actor, filter: NoteFilter) -> Option<NoteFilter> {
///         (filter.owner_id == Some(actor.user_id)).then_some(filter)
///     }
/// }
///
/// struct MyNotes<A: Adapters> {
///     crud: CrudService<A, Note, OwnNotes>,
/// }
///
/// impl<A: Adapters> MyNotes<A> {
///     fn new(ctx: Arc<Context<A>>) -> Self {
///         Self { crud: CrudService::new(ctx) }
///     }
///
///     async fn get(&self, actor: &Actor, id: NoteId) -> Result<NoteDto, AppError> {
///         self.crud.get(actor, id).await
///     }
///
///     async fn create(&self, actor: &Actor, body: CreateNoteRequest) -> Result<NoteDto, AppError> {
///         self.crud.create(actor, body).await
///     }
/// }
/// ```
pub struct CrudService<A: Adapters, E, P> {
    ctx: Arc<Context<A>>,
    types: PhantomData<fn() -> (E, P)>,
}

impl<A: Adapters, E, P> CrudService<A, E, P> {
    pub fn new(ctx: Arc<Context<A>>) -> Self {
        Self {
            ctx,
            types: PhantomData,
        }
    }

    pub fn context(&self) -> &Arc<Context<A>> {
        &self.ctx
    }
}

impl<A: Adapters, E, P> Clone for CrudService<A, E, P> {
    fn clone(&self) -> Self {
        Self::new(Arc::clone(&self.ctx))
    }
}

impl<A, E, P> CrudService<A, E, P>
where
    A: Adapters,
    E: Resource,
    P: Policy<E>,
    <A::Db as Database>::Connection: Repository<E>,
    <A::Db as Database>::Transaction: Repository<E>,
{
    pub async fn get<O: Present<E>>(&self, actor: &Actor, id: E::Id) -> Result<O, AppError> {
        let mut conn = self.ctx.db.connection().await?;
        self.get_in(&mut conn, actor, id).await
    }

    /// A page of the entities matching `filter`. `Forbidden` if the actor may not list at all, or
    /// asked for more than [`Policy::narrow`] allows.
    pub async fn list<O: Present<E>>(
        &self,
        actor: &Actor,
        filter: E::Filter,
        page: PageRequest<E::Sort>,
    ) -> Result<Page<O>, AppError> {
        let mut conn = self.ctx.db.connection().await?;
        self.list_in(&mut conn, actor, filter, page).await
    }

    /// Creates an entity with a fresh id from the clock. The actor's right to create is checked
    /// before `input` is validated.
    pub async fn create<I, O>(&self, actor: &Actor, input: I) -> Result<O, AppError>
    where
        I: Input<E::Create>,
        O: Present<E>,
    {
        let mut conn = self.ctx.db.connection().await?;
        self.create_in(&mut conn, actor, input).await
    }

    /// In a transaction: the entity is locked from the policy check to the write, so a policy that
    /// reads mutable state (a status, an assignee) cannot be raced.
    pub async fn update<I, O>(&self, actor: &Actor, id: E::Id, input: I) -> Result<O, AppError>
    where
        I: Changes<E>,
        O: Present<E>,
    {
        let mut tx = self.ctx.db.transaction().await?;
        let updated = self.update_in(&mut tx, actor, id, input).await?;
        tx.commit().await?;
        Ok(updated)
    }

    pub async fn delete(&self, actor: &Actor, id: E::Id) -> Result<(), AppError> {
        let mut tx = self.ctx.db.transaction().await?;
        self.delete_in(&mut tx, actor, id).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Loads `id` and checks that the actor may do `action` to it: `NotFound` if they may not even
    /// see it, `Forbidden` if they may see but not do it. Start every use case beyond CRUD with
    /// this.
    pub async fn authorize(&self, actor: &Actor, id: E::Id, action: Action) -> Result<E, AppError> {
        let mut conn = self.ctx.db.connection().await?;
        self.authorize_in(&mut conn, actor, id, action).await
    }

    /// [`CrudService::authorize`] on `store`. Inside a transaction, anything but `Read` locks the
    /// row until the transaction ends.
    pub async fn authorize_in<S: Store + Repository<E>>(
        &self,
        store: &mut S,
        actor: &Actor,
        id: E::Id,
        action: Action,
    ) -> Result<E, AppError> {
        let entity = if action == Action::Read {
            Repository::<E>::find_by_id(store, id).await?
        } else {
            Repository::<E>::find_by_id_for_update(store, id).await?
        }
        .ok_or(AppError::NotFound)?;
        let facts = P::facts(store, actor, Some(&entity)).await?;
        if !P::allows(actor, &facts, Action::Read, Some(&entity)) {
            return Err(AppError::NotFound);
        }
        if !P::allows(actor, &facts, action, Some(&entity)) {
            return Err(AppError::Forbidden);
        }
        Ok(entity)
    }

    pub async fn get_in<S: Store + Repository<E>, O: Present<E>>(
        &self,
        store: &mut S,
        actor: &Actor,
        id: E::Id,
    ) -> Result<O, AppError> {
        let entity = self.authorize_in(store, actor, id, Action::Read).await?;
        Ok(O::present(entity, actor))
    }

    pub async fn list_in<S: Store + Repository<E>, O: Present<E>>(
        &self,
        store: &mut S,
        actor: &Actor,
        filter: E::Filter,
        page: PageRequest<E::Sort>,
    ) -> Result<Page<O>, AppError> {
        let facts = P::facts(store, actor, None).await?;
        if !P::allows(actor, &facts, Action::Read, None) {
            return Err(AppError::Forbidden);
        }
        let filter = P::narrow(actor, &facts, filter).ok_or(AppError::Forbidden)?;

        let page = Repository::<E>::list(store, &filter, page).await?;
        Ok(page.map(|entity| O::present(entity, actor)))
    }

    pub async fn create_in<S, I, O>(
        &self,
        store: &mut S,
        actor: &Actor,
        input: I,
    ) -> Result<O, AppError>
    where
        S: Store + Repository<E>,
        I: Input<E::Create>,
        O: Present<E>,
    {
        let facts = P::facts(store, actor, None).await?;
        if !P::allows(actor, &facts, Action::Create, None) {
            return Err(AppError::Forbidden);
        }
        let create = input.into_domain(actor)?;

        let id = <E::Id as NewId>::generate_at(self.ctx.clock.now());
        let entity = Repository::<E>::create(store, id, &create).await?;
        Ok(O::present(entity, actor))
    }

    pub async fn update_in<S, I, O>(
        &self,
        store: &mut S,
        actor: &Actor,
        id: E::Id,
        input: I,
    ) -> Result<O, AppError>
    where
        S: Store + Repository<E>,
        I: Changes<E>,
        O: Present<E>,
    {
        self.update_checked_in(store, actor, id, input, |_| Ok(()))
            .await
    }

    pub async fn delete_in<S: Store + Repository<E>>(
        &self,
        store: &mut S,
        actor: &Actor,
        id: E::Id,
    ) -> Result<(), AppError> {
        self.delete_checked_in(store, actor, id, |_| Ok(())).await
    }

    /// Authorizes, runs `check` on the locked entity, validates, then writes.
    async fn update_checked_in<S, I, O>(
        &self,
        store: &mut S,
        actor: &Actor,
        id: E::Id,
        input: I,
        check: impl FnOnce(&E) -> Result<(), AppError> + Send,
    ) -> Result<O, AppError>
    where
        S: Store + Repository<E>,
        I: Changes<E>,
        O: Present<E>,
    {
        let current = self.authorize_in(store, actor, id, Action::Update).await?;
        check(&current)?;
        let changes = input.into_changes(actor, &current)?;

        let updated = Repository::<E>::update(store, id, &changes)
            .await?
            .ok_or(AppError::NotFound)?;
        Ok(O::present(updated, actor))
    }

    async fn delete_checked_in<S: Store + Repository<E>>(
        &self,
        store: &mut S,
        actor: &Actor,
        id: E::Id,
        check: impl FnOnce(&E) -> Result<(), AppError> + Send,
    ) -> Result<(), AppError> {
        let current = self.authorize_in(store, actor, id, Action::Delete).await?;
        check(&current)?;
        if Repository::<E>::delete(store, id).await? {
            Ok(())
        } else {
            Err(AppError::NotFound)
        }
    }
}

/// Optimistic concurrency for a [`Versioned`] resource: `update` and `delete` that take the
/// [`Version`] the client read and fail with `Stale` if the entity has moved on since. `None`
/// skips the check, for clients that do not send one.
///
/// The check runs on the row locked by the transaction, after authorization, so it cannot be
/// raced and an actor who may not touch the entity learns nothing from it.
impl<A, E, P> CrudService<A, E, P>
where
    A: Adapters,
    E: Resource + Versioned,
    P: Policy<E>,
    <A::Db as Database>::Connection: Repository<E>,
    <A::Db as Database>::Transaction: Repository<E>,
{
    pub async fn update_if<I, O>(
        &self,
        actor: &Actor,
        id: E::Id,
        expected: Option<Version>,
        input: I,
    ) -> Result<O, AppError>
    where
        I: Changes<E>,
        O: Present<E>,
    {
        let mut tx = self.ctx.db.transaction().await?;
        let updated = self
            .update_if_in(&mut tx, actor, id, expected, input)
            .await?;
        tx.commit().await?;
        Ok(updated)
    }

    pub async fn delete_if(
        &self,
        actor: &Actor,
        id: E::Id,
        expected: Option<Version>,
    ) -> Result<(), AppError> {
        let mut tx = self.ctx.db.transaction().await?;
        self.delete_if_in(&mut tx, actor, id, expected).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn update_if_in<S, I, O>(
        &self,
        store: &mut S,
        actor: &Actor,
        id: E::Id,
        expected: Option<Version>,
        input: I,
    ) -> Result<O, AppError>
    where
        S: Store + Repository<E>,
        I: Changes<E>,
        O: Present<E>,
    {
        self.update_checked_in(store, actor, id, input, |current| {
            ensure_version(current, expected)
        })
        .await
    }

    pub async fn delete_if_in<S: Store + Repository<E>>(
        &self,
        store: &mut S,
        actor: &Actor,
        id: E::Id,
        expected: Option<Version>,
    ) -> Result<(), AppError> {
        self.delete_checked_in(store, actor, id, |current| {
            ensure_version(current, expected)
        })
        .await
    }
}

fn ensure_version<E: Versioned>(entity: &E, expected: Option<Version>) -> Result<(), AppError> {
    match expected {
        Some(version) if version != entity.version() => Err(AppError::Stale),
        _ => Ok(()),
    }
}
