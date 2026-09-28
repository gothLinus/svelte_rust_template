//! The `Database` port on Postgres, and the executor every repository is written for.
//!
//! [`PostgresDatabase::connection`](Database::connection) and
//! [`PostgresDatabase::transaction`](Database::transaction) return a [`PgExecutor`] around a
//! pooled connection or an open sqlx transaction. Because the repositories are implemented for
//! `PgExecutor<C>` generically, a service written against `Adapters::Db` runs the same code
//! either way; dropping an uncommitted transaction rolls it back.

use std::ops::DerefMut;

use domain::database::{Database, StorageError, Transaction};
use sqlx::{Connection, PgConnection, PgPool, Postgres, pool::PoolConnection};

use crate::db::errors::db_error;

#[derive(Debug, Clone)]
pub struct PostgresDatabase {
    pool: PgPool,
}

impl PostgresDatabase {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

/// Anything that derefs to a live connection: a pooled connection or an open transaction.
///
/// This is the bound on every repository impl (`impl<C: PgHandle> ... for PgExecutor<C>`). It is
/// blanket-implemented, so there is nothing to implement by hand.
pub trait PgHandle: DerefMut<Target = PgConnection> + Send {}

impl<C> PgHandle for C where C: DerefMut<Target = PgConnection> + Send {}

/// The unit of work. Every repository trait is implemented once, for `PgExecutor<C>` with any
/// [`PgHandle`], so the same code runs on a plain connection and inside a transaction, and no sqlx
/// type leaks into the traits.
///
/// `C` is a `PoolConnection<Postgres>` (from [`Database::connection`]) or an
/// `sqlx::Transaction<'static, Postgres>` (from [`Database::transaction`]; only this one is a
/// [`Transaction`] and can commit). A repository method borrows the underlying connection with
/// `self.conn()` and passes it to sqlx.
///
/// Every sqlx error goes through `db_error` so it becomes a `StorageError`. Statements in a
/// repository method run in the caller's transaction if there is one; a method never begins or
/// commits its own.
pub struct PgExecutor<C>(C);

impl<C: PgHandle> PgExecutor<C> {
    pub(crate) fn conn(&mut self) -> &mut PgConnection {
        &mut self.0
    }
}

impl Database for PostgresDatabase {
    type Connection = PgExecutor<PoolConnection<Postgres>>;
    type Transaction = PgExecutor<sqlx::Transaction<'static, Postgres>>;

    async fn connection(&self) -> Result<Self::Connection, StorageError> {
        self.pool.acquire().await.map(PgExecutor).map_err(db_error)
    }

    async fn transaction(&self) -> Result<Self::Transaction, StorageError> {
        self.pool.begin().await.map(PgExecutor).map_err(db_error)
    }

    async fn ping(&self) -> Result<(), StorageError> {
        let mut conn = self.pool.acquire().await.map_err(db_error)?;
        conn.ping().await.map_err(db_error)
    }
}

impl Transaction for PgExecutor<sqlx::Transaction<'static, Postgres>> {
    async fn commit(self) -> Result<(), StorageError> {
        self.0.commit().await.map_err(db_error)
    }
}
