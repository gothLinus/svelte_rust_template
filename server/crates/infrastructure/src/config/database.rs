use std::{str::FromStr, time::Duration};

use domain::secret::Secret;
use sqlx::postgres::PgConnectOptions;

use super::reader::Reader;

#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub url: Secret,
    pub max_connections: u32,
    pub acquire_timeout: Duration,
    /// The longest a statement may run, and a transaction may sit idle, before Postgres cancels it.
    /// A request that times out stops waiting for its query, but the query keeps running, and
    /// holding its locks, unless the server ends it. Zero turns it off.
    pub statement_timeout: Duration,
    pub run_migrations: bool,
}

impl Reader<'_> {
    pub(super) fn database(&mut self) -> Option<DatabaseConfig> {
        let url = self.required("DATABASE_URL", |raw| {
            let postgres = raw.starts_with("postgres://") || raw.starts_with("postgresql://");
            match PgConnectOptions::from_str(raw) {
                Ok(_) if postgres => Ok(Secret::new(raw)),
                _ => Err("must be a postgres:// URL".to_owned()),
            }
        });
        let max_connections = self.number("DATABASE_MAX_CONNECTIONS", 10);
        let acquire_timeout = self.duration(
            "DATABASE_ACQUIRE_TIMEOUT",
            Duration::from_secs(5),
            Duration::from_mins(10),
        );
        let statement_timeout = self.duration(
            "DATABASE_STATEMENT_TIMEOUT",
            Duration::from_secs(30),
            Duration::from_hours(1),
        );
        let run_migrations = self.bool("DATABASE_RUN_MIGRATIONS", true);

        if max_connections == 0 {
            self.problem("DATABASE_MAX_CONNECTIONS", "must be at least 1");
        }

        Some(DatabaseConfig {
            url: url?,
            max_connections,
            acquire_timeout,
            statement_timeout,
            run_migrations,
        })
    }
}
