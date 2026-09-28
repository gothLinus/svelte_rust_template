//! PostgreSQL through sqlx: the connection pool, migrations, the unit-of-work adapter and every
//! repository.
//!
//! - [`PostgresDatabase`] implements `domain::database::Database`: it hands out pooled connections
//!   and transactions, both wrapped in [`PgExecutor`].
//! - Every repository trait from `domain` is implemented once, for `PgExecutor<C>` with any
//!   [`PgHandle`] (`repositories/notes.rs` is the reference to copy), so the same query code runs
//!   inside and outside a transaction.
//! - sqlx errors become `domain::error::StorageError` at this boundary, so constraint violations
//!   can be handled by name and no sqlx type reaches the services.
//! - [`connect`] and [`migrate`] set up the pool; [`ClusterLock`] serializes work across
//!   instances.
//!
//! Queries are checked against the schema at compile time (`query!`/`query_as!`). Builds without a
//! database use the committed `.sqlx/` metadata (`SQLX_OFFLINE=true`); regenerate it with
//! `just sqlx-prepare` after changing a query.

pub use lock::ClusterLock;
pub use pool::{MIGRATOR, PoolError, connect, migrate};
pub use postgres::{PgExecutor, PgHandle, PostgresDatabase};
pub use sqlx::PgPool;

pub(crate) mod errors;
mod lock;
mod pool;
mod postgres;
pub(crate) mod repositories;
