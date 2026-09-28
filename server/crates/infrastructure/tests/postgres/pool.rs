use std::time::Duration;

use domain::secret::Secret;
use infrastructure::{config::DatabaseConfig, db};
use sqlx::PgPool;

pub fn url_of(pool: &PgPool) -> String {
    let base = std::env::var("DATABASE_URL").unwrap();
    let database = pool.connect_options().get_database().unwrap().to_owned();
    let (server, _) = base.rsplit_once('/').unwrap();
    format!("{server}/{database}")
}

fn config(url: String) -> DatabaseConfig {
    DatabaseConfig {
        url: Secret::new(url),
        max_connections: 2,
        acquire_timeout: Duration::from_secs(5),
        statement_timeout: Duration::from_secs(30),
        run_migrations: true,
    }
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn connect_and_migrate(pool: PgPool) {
    let connected = db::connect(&config(url_of(&pool))).await.unwrap();

    db::migrate(&connected).await.unwrap();
    let applied: i64 = sqlx::query_scalar("select count(*) from _sqlx_migrations")
        .fetch_one(&connected)
        .await
        .unwrap();
    assert_eq!(
        applied,
        i64::try_from(
            db::MIGRATOR
                .iter()
                .filter(|migration| !migration.migration_type.is_down_migration())
                .count()
        )
        .unwrap()
    );
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn connections_carry_the_statement_timeout(pool: PgPool) {
    let connected = db::connect(&config(url_of(&pool))).await.unwrap();

    let (statement, idle): (String, String) = sqlx::query_as(
        "select current_setting('statement_timeout'), \
                current_setting('idle_in_transaction_session_timeout')",
    )
    .fetch_one(&connected)
    .await
    .unwrap();
    assert_eq!((statement.as_str(), idle.as_str()), ("30s", "30s"));

    let cancelled = sqlx::query("select pg_sleep(1)")
        .execute(
            &db::connect(&DatabaseConfig {
                statement_timeout: Duration::from_millis(100),
                ..config(url_of(&pool))
            })
            .await
            .unwrap(),
        )
        .await
        .unwrap_err();
    assert_eq!(
        cancelled
            .as_database_error()
            .and_then(sqlx::error::DatabaseError::code)
            .as_deref(),
        Some("57014")
    );
}

#[tokio::test]
async fn connection_errors_are_reported() {
    assert!(matches!(
        db::connect(&config("postgres://nobody@127.0.0.1:1/none".to_owned())).await,
        Err(db::PoolError::Connect(_))
    ));
    assert!(matches!(
        db::connect(&config("not a url".to_owned())).await,
        Err(db::PoolError::InvalidUrl(_))
    ));
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn a_cluster_lock_lets_one_instance_run_at_a_time(pool: sqlx::PgPool) {
    use infrastructure::db::ClusterLock;

    let first = ClusterLock::new(pool.clone(), 42);
    let second = ClusterLock::new(pool.clone(), 42);

    let ran = first
        .run_if_free(async {
            assert!(!second.run_if_free(async {}).await.unwrap());
        })
        .await
        .unwrap();
    assert!(ran);
    assert!(second.run_if_free(async {}).await.unwrap());
}
