use std::{num::NonZeroU32, time::Duration};

use infrastructure::rate_limit::{BucketStore, Limited, Rate};
use sqlx::PgPool;

fn two_per_two_hours() -> Rate {
    Rate::new(NonZeroU32::new(2).unwrap(), Duration::from_hours(2))
}

fn stores(pool: &PgPool) -> [BucketStore; 2] {
    [BucketStore::memory(), BucketStore::Postgres(pool.clone())]
}

async fn take(store: &BucketStore, key: &str) -> Result<(), Limited> {
    store
        .take("login_ip", key, two_per_two_hours())
        .await
        .unwrap()
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn both_stores_allow_the_burst_then_refuse(pool: PgPool) {
    for store in stores(&pool) {
        assert_eq!(take(&store, "198.51.100.1").await, Ok(()));
        assert_eq!(take(&store, "198.51.100.1").await, Ok(()));

        let Err(Limited { retry_after }) = take(&store, "198.51.100.1").await else {
            panic!("the third request passed");
        };
        assert!(
            (Duration::from_mins(59)..=Duration::from_hours(1)).contains(&retry_after),
            "{retry_after:?}"
        );

        assert_eq!(take(&store, "198.51.100.2").await, Ok(()));
        let other = store
            .take("register_ip", "198.51.100.1", two_per_two_hours())
            .await
            .unwrap();
        assert_eq!(other, Ok(()));
    }
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn a_request_given_back_is_available_again(pool: PgPool) {
    for store in stores(&pool) {
        take(&store, "alice").await.unwrap();
        take(&store, "alice").await.unwrap();
        store
            .give_back("login_ip", "alice", two_per_two_hours())
            .await
            .unwrap();

        assert_eq!(take(&store, "alice").await, Ok(()));
        assert!(take(&store, "alice").await.is_err());
    }
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn instances_sharing_postgres_share_the_budget(pool: PgPool) {
    let (first, second) = (
        BucketStore::Postgres(pool.clone()),
        BucketStore::Postgres(pool.clone()),
    );

    let (a, b, c, d) = tokio::join!(
        take(&first, "bob"),
        take(&second, "bob"),
        take(&first, "bob"),
        take(&second, "bob"),
    );
    assert_eq!([a, b, c, d].iter().filter(|taken| taken.is_ok()).count(), 2);
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn full_buckets_are_forgotten(pool: PgPool) {
    let store = BucketStore::Postgres(pool.clone());
    take(&store, "carol").await.unwrap();
    sqlx::query("insert into rate_limits (bucket, key, tat) values ('login_ip', 'dave', now())")
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(store.retain_recent().await.unwrap(), 1);
    let left: Vec<String> = sqlx::query_scalar("select key from rate_limits")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert_eq!(left, ["carol"]);
}
