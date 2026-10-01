//! The generic [`Repository`] for files on Postgres, and [`FileRepository`]. Built like
//! `notes.rs` (see there for the pattern); what differs is the row's object key and the sum of
//! sizes the quota checks.
//!
//! Deleting a row queues its object for removal from the store in the same statement: the
//! migration's `files_queue_object_deletion` trigger, which also covers rows removed by the
//! cascade from `users`. `lock_storage` takes a transaction-scoped advisory lock per owner, so
//! concurrent uploads cannot each fit the quota and together pass it.

use domain::{
    error::StorageError,
    file::{
        ContentType, FileChanges, FileFilter, FileId, FileName, FileRepository, FileSize, NewFile,
        StoredFile, StoredFileParts,
    },
    object_store::ObjectKey,
    pagination::{Cursor, NewestFirst, Page, PageRequest},
    repository::Repository,
    user::UserId,
};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::db::{
    errors::{corrupt, db_error, to_i64, to_u64},
    postgres::{PgExecutor, PgHandle},
};

/// The first half of the advisory lock key behind [`FileRepository::lock_storage`]; the second is
/// a hash of the owner's id. Two-key locks never collide with the one-key locks elsewhere (such as
/// `ROLE_ASSIGNMENT_LOCK`).
const STORAGE_LOCK_SPACE: i32 = 0x6669_6c65;

struct FileRow {
    id: Uuid,
    owner_id: Uuid,
    name: String,
    content_type: String,
    size: i64,
    object_key: String,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

impl TryFrom<FileRow> for StoredFile {
    type Error = StorageError;

    fn try_from(row: FileRow) -> Result<Self, Self::Error> {
        Ok(Self::from_parts(StoredFileParts {
            id: FileId::from_uuid(row.id),
            owner_id: UserId::from_uuid(row.owner_id),
            name: FileName::parse(&row.name).map_err(corrupt)?,
            content_type: ContentType::parse(&row.content_type).map_err(corrupt)?,
            size: FileSize::new(to_u64(row.size)?).map_err(corrupt)?,
            object_key: ObjectKey::parse(&row.object_key).map_err(corrupt)?,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }))
    }
}

fn size(size: FileSize) -> Result<i64, StorageError> {
    i64::try_from(size.bytes()).map_err(corrupt)
}

impl<C: PgHandle> Repository<StoredFile> for PgExecutor<C> {
    async fn find_by_id(&mut self, id: FileId) -> Result<Option<StoredFile>, StorageError> {
        sqlx::query_as!(
            FileRow,
            r#"
            select id, owner_id, name, content_type, size, object_key, created_at, updated_at
            from files
            where id = $1
            "#,
            id.as_uuid(),
        )
        .fetch_optional(self.conn())
        .await
        .map_err(db_error)?
        .map(StoredFile::try_from)
        .transpose()
    }

    async fn find_by_id_for_update(
        &mut self,
        id: FileId,
    ) -> Result<Option<StoredFile>, StorageError> {
        sqlx::query_as!(
            FileRow,
            r#"
            select id, owner_id, name, content_type, size, object_key, created_at, updated_at
            from files
            where id = $1
            for update
            "#,
            id.as_uuid(),
        )
        .fetch_optional(self.conn())
        .await
        .map_err(db_error)?
        .map(StoredFile::try_from)
        .transpose()
    }

    async fn list(
        &mut self,
        filter: &FileFilter,
        page: PageRequest<NewestFirst>,
    ) -> Result<Page<StoredFile>, StorageError> {
        let after = page.after_id().unwrap_or(Uuid::max());
        let limit = to_i64(page.fetch_limit());
        let rows = match filter.owner_id {
            Some(owner) => {
                sqlx::query_as!(
                    FileRow,
                    r#"
                    select id, owner_id, name, content_type, size, object_key, created_at,
                        updated_at
                    from files
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
                    FileRow,
                    r#"
                    select id, owner_id, name, content_type, size, object_key, created_at,
                        updated_at
                    from files
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
        .map(StoredFile::try_from)
        .collect::<Result<Vec<_>, _>>()?;

        Ok(Page::from_rows(rows, &page, |file| {
            Cursor::from_uuid(file.id().as_uuid())
        }))
    }

    async fn create(&mut self, id: FileId, input: &NewFile) -> Result<StoredFile, StorageError> {
        sqlx::query_as!(
            FileRow,
            r#"
            insert into files (id, owner_id, name, content_type, size, object_key)
            values ($1, $2, $3, $4, $5, $6)
            returning id, owner_id, name, content_type, size, object_key, created_at,
                updated_at
            "#,
            id.as_uuid(),
            input.owner_id.as_uuid(),
            input.name.as_str(),
            input.content_type.as_str(),
            size(input.size)?,
            input.object_key.as_str(),
        )
        .fetch_one(self.conn())
        .await
        .map_err(db_error)?
        .try_into()
    }

    async fn update(
        &mut self,
        id: FileId,
        changes: &FileChanges,
    ) -> Result<Option<StoredFile>, StorageError> {
        sqlx::query_as!(
            FileRow,
            r#"
            update files
            set name = coalesce($2, name)
            where id = $1
            returning id, owner_id, name, content_type, size, object_key, created_at,
                updated_at
            "#,
            id.as_uuid(),
            changes.name.as_ref().map(FileName::as_str),
        )
        .fetch_optional(self.conn())
        .await
        .map_err(db_error)?
        .map(StoredFile::try_from)
        .transpose()
    }

    async fn delete(&mut self, id: FileId) -> Result<bool, StorageError> {
        let result = sqlx::query!("delete from files where id = $1", id.as_uuid())
            .execute(self.conn())
            .await
            .map_err(db_error)?;

        Ok(result.rows_affected() > 0)
    }
}

impl<C: PgHandle> FileRepository for PgExecutor<C> {
    /// An index-only scan of `files_owner_id_id_idx`, which includes the size.
    async fn stored_bytes(&mut self, owner: UserId) -> Result<u64, StorageError> {
        let total = sqlx::query_scalar!(
            r#"select coalesce(sum(size), 0)::bigint as "total!" from files where owner_id = $1"#,
            owner.as_uuid(),
        )
        .fetch_one(self.conn())
        .await
        .map_err(db_error)?;
        to_u64(total)
    }

    async fn lock_storage(&mut self, owner: UserId) -> Result<(), StorageError> {
        // Transaction-scoped, like `lock_role_assignments`: released on commit or rollback. Owners
        // whose ids hash alike share a lock, which only makes them wait. Transaction-scoped, like
        // `lock_role_assignments`: released on commit or rollback. Owners whose ids hash alike
        // share a lock, which only makes them wait.
        sqlx::query!(
            r#"
            select true as "locked!"
            from (select pg_advisory_xact_lock($1, hashtext($2::uuid::text))) as lock
            "#,
            STORAGE_LOCK_SPACE,
            owner.as_uuid(),
        )
        .fetch_one(self.conn())
        .await
        .map_err(db_error)?;

        Ok(())
    }
}
