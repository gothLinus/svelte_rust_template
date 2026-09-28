use std::str::FromStr;

use sqlx::{
    Connection, PgPool,
    migrate::{MigrateError, Migrator},
    postgres::{PgConnectOptions, PgPoolOptions},
};
use thiserror::Error;

use crate::config::DatabaseConfig;

pub static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

#[derive(Debug, Error)]
pub enum PoolError {
    #[error("DATABASE_URL is invalid")]
    InvalidUrl(#[source] sqlx::Error),
    #[error("failed to connect to the database")]
    Connect(#[source] sqlx::Error),
    #[error("failed to run the database migrations")]
    Migrate(#[source] MigrateError),
}

/// Opens the pool and checks that the database answers.
///
/// Every connection gets `statement_timeout` and `idle_in_transaction_session_timeout` set to
/// `config.statement_timeout`, so a stuck query or an abandoned transaction cannot hold a
/// connection or its locks forever.
pub async fn connect(config: &DatabaseConfig) -> Result<PgPool, PoolError> {
    let timeout = config.statement_timeout.as_millis().to_string();
    let options = PgConnectOptions::from_str(config.url.expose())
        .map_err(PoolError::InvalidUrl)?
        .options([
            ("statement_timeout", timeout.as_str()),
            ("idle_in_transaction_session_timeout", timeout.as_str()),
        ]);
    PgPoolOptions::new()
        .max_connections(config.max_connections)
        .acquire_timeout(config.acquire_timeout)
        .connect_with(options)
        .await
        .map_err(PoolError::Connect)
}

/// Applies pending migrations. Safe to run from several instances at once: sqlx takes an advisory
/// lock.
///
/// They run on a connection taken out of the pool, without the statement timeout: a migration may
/// rewrite a large table, and waiting for another instance's advisory lock is a statement too. The
/// connection is closed afterwards instead of going back.
pub async fn migrate(pool: &PgPool) -> Result<(), PoolError> {
    let mut conn = pool.acquire().await.map_err(PoolError::Connect)?.detach();
    sqlx::query("set statement_timeout = 0")
        .execute(&mut conn)
        .await
        .map_err(PoolError::Connect)?;
    let result = MIGRATOR.run(&mut conn).await.map_err(PoolError::Migrate);
    // The migrations' outcome is what matters; a failed goodbye only loses a connection.
    let _ = conn.close().await;
    result
}
