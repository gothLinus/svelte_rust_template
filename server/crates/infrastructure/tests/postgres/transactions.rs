use domain::{
    database::{Database, Transaction},
    user::{Email, UserRepository},
};
use infrastructure::db::PostgresDatabase;
use sqlx::PgPool;

use crate::support::new_user;

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn committed_work_is_kept(pool: PgPool) {
    let db = PostgresDatabase::new(pool);

    let mut tx = db.transaction().await.unwrap();
    tx.create_user(&new_user("alice@example.com"))
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let found = db
        .connection()
        .await
        .unwrap()
        .find_user_by_email(&Email::parse("alice@example.com").unwrap())
        .await
        .unwrap();
    assert!(found.is_some());
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn dropping_a_transaction_rolls_it_back(pool: PgPool) {
    let db = PostgresDatabase::new(pool);

    {
        let mut tx = db.transaction().await.unwrap();
        tx.create_user(&new_user("alice@example.com"))
            .await
            .unwrap();
        let inside = tx
            .find_user_by_email(&Email::parse("alice@example.com").unwrap())
            .await
            .unwrap();
        assert!(inside.is_some());
    }

    let after = db
        .connection()
        .await
        .unwrap()
        .find_user_by_email(&Email::parse("alice@example.com").unwrap())
        .await
        .unwrap();
    assert!(after.is_none());
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn ping(pool: PgPool) {
    let db = PostgresDatabase::new(pool.clone());
    db.ping().await.unwrap();
    assert!(db.pool().size() >= 1);

    pool.close().await;
    assert!(db.ping().await.is_err());
}
