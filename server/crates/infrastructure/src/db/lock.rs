use domain::error::StorageError;
use sqlx::PgPool;

use crate::db::errors::db_error;

/// A Postgres session advisory lock under a fixed key. [`ClusterLock::run_if_free`] runs a task
/// only on the instance that got the lock; the others skip that round.
///
/// Use a distinct `key` per kind of work; two locks with the same key exclude each other.
#[derive(Debug, Clone)]
pub struct ClusterLock {
    pool: PgPool,
    key: i64,
}

impl ClusterLock {
    /// Periodic maintenance (expired rows, retention): harmless to repeat, but pointless to run on
    /// every replica.
    pub const MAINTENANCE: i64 = 0x6d61_696e_7400;

    pub fn new(pool: PgPool, key: i64) -> Self {
        Self { pool, key }
    }

    /// Runs `task` if no other instance holds the lock, and returns whether it ran. The lock
    /// belongs to one pooled connection for as long as `task` runs; if the process dies, Postgres
    /// releases it with the connection.
    ///
    /// # Errors
    ///
    /// Fails if the pool cannot supply a connection or the lock statements fail. A failed unlock
    /// leaves the lock held by that connection until it is closed.
    pub async fn run_if_free<F: Future<Output = ()>>(&self, task: F) -> Result<bool, StorageError> {
        let mut conn = self.pool.acquire().await.map_err(db_error)?;
        let locked: bool = sqlx::query_scalar("select pg_try_advisory_lock($1)")
            .bind(self.key)
            .fetch_one(&mut *conn)
            .await
            .map_err(db_error)?;
        if !locked {
            return Ok(false);
        }
        task.await;
        sqlx::query("select pg_advisory_unlock($1)")
            .bind(self.key)
            .execute(&mut *conn)
            .await
            .map_err(db_error)?;
        Ok(true)
    }
}
