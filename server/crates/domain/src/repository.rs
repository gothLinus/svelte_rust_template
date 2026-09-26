//! The generic repository port every CRUD resource implements.
//!
//! Adding a resource takes two impls: [`Resource`] for the entity (see [`crate::note`]) and
//! [`Repository`] for the adapter's connection type (see
//! `infrastructure::db::repositories::notes`). `application::crud::CrudService` then works for it
//! without further code.

use std::{fmt::Debug, future::Future};

use crate::{
    error::StorageError,
    id::NewId,
    pagination::{Page, PageRequest, Sort},
};

/// An entity the generic [`Repository`] can store; the associated types name the inputs and
/// outputs of its CRUD operations.
///
/// They hold validated domain values, so nothing behind a repository re-validates them.
///
/// ```
/// use domain::{id::Id, pagination::NewestFirst, repository::Resource};
///
/// struct Tag;
/// struct NewTag { name: String }
/// struct TagChanges { name: Option<String> }
///
/// impl Resource for Tag {
///     type Id = Id<Tag>;
///     type Create = NewTag;
///     type Update = TagChanges;
///     type Filter = ();
///     type Sort = NewestFirst;
/// }
/// ```
pub trait Resource: Sized + Send + Sync + 'static {
    type Id: Copy + Debug + Send + Sync + NewId;
    type Create: Send + Sync;
    type Update: Send + Sync;
    type Filter: Default + Send + Sync;
    type Sort: Sort;
}

/// Basic persistence for a [`Resource`]. Entity-specific queries go into their own traits
/// next to the entity, implemented on the same connection type.
///
/// Concrete code with several `Repository<E>` impls in scope must name the entity:
/// `Repository::<Note>::find_by_id(&mut conn, id)`.
///
/// Implement it once per entity for the adapter's connection type, which serves a pooled
/// connection and a transaction alike. Failures are [`StorageError`]s, never driver errors;
/// a missing row is `None` or `false`, not an error.
///
/// # Examples
///
/// A read-check-write that must not race with other writers:
///
/// ```no_run
/// use domain::{
///     error::StorageError,
///     repository::{Repository, Resource},
/// };
///
/// /// Applies `changes` to the entity if `allowed` accepts its current state.
/// async fn update_if<E, R>(
///     repo: &mut R,
///     id: E::Id,
///     allowed: impl FnOnce(&E) -> bool,
///     changes: &E::Update,
/// ) -> Result<Option<E>, StorageError>
/// where
///     E: Resource,
///     R: Repository<E>,
/// {
///     let Some(current) = repo.find_by_id_for_update(id).await? else {
///         return Ok(None);
///     };
///     if !allowed(&current) {
///         return Ok(None);
///     }
///     repo.update(id, changes).await
/// }
/// ```
#[diagnostic::on_unimplemented(
    message = "`{Self}` has no `Repository<{E}>`",
    label = "no `Repository<{E}>` here",
    note = "add `Repository<{E}>` to `application::Store` (context.rs: the trait and its blanket impl), implement it for `PgExecutor<C>` in infrastructure/src/db/repositories/, and for the in-memory fake in the application tests"
)]
pub trait Repository<E: Resource>: Send {
    fn find_by_id(
        &mut self,
        id: E::Id,
    ) -> impl Future<Output = Result<Option<E>, StorageError>> + Send;

    /// Like [`Repository::find_by_id`], but inside a transaction the row stays locked until it
    /// ends (`select … for update` on Postgres), so what a policy checked cannot change before the
    /// following write. Backends that cannot lock fall back to a plain read. Outside a transaction
    /// the lock is released at once.
    fn find_by_id_for_update(
        &mut self,
        id: E::Id,
    ) -> impl Future<Output = Result<Option<E>, StorageError>> + Send {
        self.find_by_id(id)
    }

    fn list(
        &mut self,
        filter: &E::Filter,
        page: PageRequest<E::Sort>,
    ) -> impl Future<Output = Result<Page<E>, StorageError>> + Send;

    fn create(
        &mut self,
        id: E::Id,
        input: &E::Create,
    ) -> impl Future<Output = Result<E, StorageError>> + Send;

    fn update(
        &mut self,
        id: E::Id,
        changes: &E::Update,
    ) -> impl Future<Output = Result<Option<E>, StorageError>> + Send;

    fn delete(&mut self, id: E::Id) -> impl Future<Output = Result<bool, StorageError>> + Send;
}
