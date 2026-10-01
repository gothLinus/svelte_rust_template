//! [`ObjectDeletionRepository`] on Postgres: the `object_deletions` table, the queue of objects to
//! remove from the object store. Deleted files arrive through the migration's trigger (`due_at`
//! null); uploads in progress queue their own key with a time.

use domain::{
    error::StorageError,
    object_store::{ObjectDeletionRepository, ObjectKey},
};
use time::OffsetDateTime;

use crate::db::{
    errors::{corrupt, db_error, to_i64},
    postgres::{PgExecutor, PgHandle},
};

impl<C: PgHandle> ObjectDeletionRepository for PgExecutor<C> {
    async fn schedule_object_deletion(
        &mut self,
        key: &ObjectKey,
        due_at: Option<OffsetDateTime>,
    ) -> Result<(), StorageError> {
        sqlx::query!(
            r#"
            insert into object_deletions (object_key, due_at)
            values ($1, $2)
            on conflict (object_key) do update set due_at = excluded.due_at
            "#,
            key.as_str(),
            due_at,
        )
        .execute(self.conn())
        .await
        .map_err(db_error)?;
        Ok(())
    }

    async fn cancel_object_deletion(&mut self, key: &ObjectKey) -> Result<bool, StorageError> {
        let result = sqlx::query!(
            "delete from object_deletions where object_key = $1",
            key.as_str(),
        )
        .execute(self.conn())
        .await
        .map_err(db_error)?;
        Ok(result.rows_affected() > 0)
    }

    async fn due_object_deletions(
        &mut self,
        now: OffsetDateTime,
        limit: u32,
    ) -> Result<Vec<ObjectKey>, StorageError> {
        sqlx::query_scalar!(
            r#"
            select object_key
            from object_deletions
            where due_at is null or due_at <= $1
            order by due_at nulls first
            limit $2
            "#,
            now,
            to_i64(limit),
        )
        .fetch_all(self.conn())
        .await
        .map_err(db_error)?
        .iter()
        .map(|key| ObjectKey::parse(key).map_err(corrupt))
        .collect()
    }
}
