//! The generic [`Repository`] for notes on Postgres: the reference to copy when adding a resource
//! (`just new-resource` copies it together with every other layer).
//!
//! The slice consists of:
//!
//! - `domain::note`: the entity, its `Resource` impl and its validated value types.
//! - `application::notes`: the DTOs, the policy and the `CrudService` that call this repository.
//! - this file, plus the migration `migrations/*_notes.up.sql` (table, the index the list query
//!   relies on, the `set_updated_at` trigger).
//! - `api` routes and wire conversions (`api::routes::notes`, `api::wire::notes`) and
//!   `proto/api/v1/notes.proto`.
//!
//! To write the impl for a new entity:
//!
//! 1. Define a private row struct with the plain column types and a `TryFrom<Row>` for the entity
//!    that re-validates each value with the type's `parse`, mapping failures with `corrupt` (a bad
//!    stored value is a data error, not a client error).
//! 2. Implement `Repository<Entity>` for `PgExecutor<C: PgHandle>`, running each query on
//!    `self.conn()` and ending it with `.map_err(db_error)`.
//! 3. `find_by_id_for_update` is `find_by_id` plus `for update`: the service reads with it inside a
//!    transaction before checking the policy, so the row cannot change under it.
//! 4. `list` is keyset pagination with a static query per filter shape; see the method. A resource
//!    with orders other than `NewestFirst` matches on `page.sort` and builds its cursors with
//!    `Cursor::keyset`.
//! 5. `create` inserts with the id the caller generated and returns the stored row; `update`
//!    applies the present fields with `coalesce` and returns `None` if the row is gone; `delete`
//!    reports whether a row was deleted.
//! 6. Queries are checked at compile time. After adding or changing one, run `just sqlx-prepare`
//!    and commit the updated `.sqlx/` cache so builds without a database still work.

use domain::{
    error::StorageError,
    note::{NewNote, Note, NoteBody, NoteChanges, NoteFilter, NoteId, NoteParts, NoteTitle},
    pagination::{Cursor, NewestFirst, Page, PageRequest},
    repository::Repository,
    user::UserId,
};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::db::{
    errors::{corrupt, db_error, to_i64},
    postgres::{PgExecutor, PgHandle},
};

struct NoteRow {
    id: Uuid,
    owner_id: Uuid,
    title: String,
    body: String,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

impl TryFrom<NoteRow> for Note {
    type Error = StorageError;

    fn try_from(row: NoteRow) -> Result<Self, Self::Error> {
        Ok(Self::from_parts(NoteParts {
            id: NoteId::from_uuid(row.id),
            owner_id: UserId::from_uuid(row.owner_id),
            title: NoteTitle::parse(&row.title).map_err(corrupt)?,
            body: NoteBody::parse(&row.body).map_err(corrupt)?,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }))
    }
}

impl<C: PgHandle> Repository<Note> for PgExecutor<C> {
    async fn find_by_id(&mut self, id: NoteId) -> Result<Option<Note>, StorageError> {
        sqlx::query_as!(
            NoteRow,
            r#"
            select id, owner_id, title, body, created_at, updated_at
            from notes
            where id = $1
            "#,
            id.as_uuid(),
        )
        .fetch_optional(self.conn())
        .await
        .map_err(db_error)?
        .map(Note::try_from)
        .transpose()
    }

    async fn find_by_id_for_update(&mut self, id: NoteId) -> Result<Option<Note>, StorageError> {
        sqlx::query_as!(
            NoteRow,
            r#"
            select id, owner_id, title, body, created_at, updated_at
            from notes
            where id = $1
            for update
            "#,
            id.as_uuid(),
        )
        .fetch_optional(self.conn())
        .await
        .map_err(db_error)?
        .map(Note::try_from)
        .transpose()
    }

    /// Keyset pagination, newest first: `order by id desc` with `id < $cursor`, fetching
    /// `page.fetch_limit()` rows (one more than the page holds) and letting [`Page::from_rows`]
    /// turn the extra row into the next [`Cursor`].
    ///
    /// One static query per filter shape, not `($1 is null or owner_id = $1)`: Postgres may cache a
    /// generic plan for a prepared statement, and for the catch-all form that plan scans the
    /// primary key and filters (547 ms on a million notes, against 0.08 ms with the owner index).
    /// The first page starts after the largest UUID, so the cursor is always a plain range
    /// condition. Copy this shape, not a catch-all.
    async fn list(
        &mut self,
        filter: &NoteFilter,
        page: PageRequest<NewestFirst>,
    ) -> Result<Page<Note>, StorageError> {
        let after = page.after_id().unwrap_or(Uuid::max());
        let limit = to_i64(page.fetch_limit());
        let rows = match filter.owner_id {
            Some(owner) => {
                sqlx::query_as!(
                    NoteRow,
                    r#"
                    select id, owner_id, title, body, created_at, updated_at
                    from notes
                    where owner_id = $1 and id < $2
                    order by id desc
                    limit $3
                    "#,
                    owner.as_uuid(),
                    after,
                    limit,
                )
                .fetch_all(self.conn())
                .await
            }
            None => {
                sqlx::query_as!(
                    NoteRow,
                    r#"
                    select id, owner_id, title, body, created_at, updated_at
                    from notes
                    where id < $1
                    order by id desc
                    limit $2
                    "#,
                    after,
                    limit,
                )
                .fetch_all(self.conn())
                .await
            }
        }
        .map_err(db_error)?
        .into_iter()
        .map(Note::try_from)
        .collect::<Result<Vec<_>, _>>()?;

        Ok(Page::from_rows(rows, &page, |note| {
            Cursor::from_uuid(note.id().as_uuid())
        }))
    }

    async fn create(&mut self, id: NoteId, input: &NewNote) -> Result<Note, StorageError> {
        sqlx::query_as!(
            NoteRow,
            r#"
            insert into notes (id, owner_id, title, body)
            values ($1, $2, $3, $4)
            returning id, owner_id, title, body, created_at, updated_at
            "#,
            id.as_uuid(),
            input.owner_id.as_uuid(),
            input.title.as_str(),
            input.body.as_str(),
        )
        .fetch_one(self.conn())
        .await
        .map_err(db_error)?
        .try_into()
    }

    async fn update(
        &mut self,
        id: NoteId,
        changes: &NoteChanges,
    ) -> Result<Option<Note>, StorageError> {
        sqlx::query_as!(
            NoteRow,
            r#"
            update notes
            set title = coalesce($2, title), body = coalesce($3, body)
            where id = $1
            returning id, owner_id, title, body, created_at, updated_at
            "#,
            id.as_uuid(),
            changes.title.as_ref().map(NoteTitle::as_str),
            changes.body.as_ref().map(NoteBody::as_str),
        )
        .fetch_optional(self.conn())
        .await
        .map_err(db_error)?
        .map(Note::try_from)
        .transpose()
    }

    async fn delete(&mut self, id: NoteId) -> Result<bool, StorageError> {
        let result = sqlx::query!("delete from notes where id = $1", id.as_uuid())
            .execute(self.conn())
            .await
            .map_err(db_error)?;

        Ok(result.rows_affected() > 0)
    }
}
