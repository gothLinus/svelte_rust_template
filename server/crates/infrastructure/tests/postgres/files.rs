use domain::{
    database::{Database, Transaction},
    file::{
        ContentType, FileChanges, FileFilter, FileId, FileName, FileRepository, FileSize, NewFile,
        StoredFile, new_object_key,
    },
    object_store::{ObjectDeletionRepository, ObjectKey},
    pagination::{PageRequest, PageSize},
    repository::Repository,
    user::{User, UserRepository},
};
use infrastructure::db::PostgresDatabase;
use sqlx::PgPool;
use time::{Duration, OffsetDateTime};

use crate::support::{Conn, conn, user};

async fn file(conn: &mut Conn, owner: &User, name: &str, size: u64) -> StoredFile {
    Repository::<StoredFile>::create(
        conn,
        FileId::generate(),
        &NewFile {
            owner_id: owner.id(),
            name: FileName::parse(name).unwrap(),
            content_type: ContentType::parse("text/plain").unwrap(),
            size: FileSize::new(size).unwrap(),
            object_key: new_object_key(),
        },
    )
    .await
    .unwrap()
}

async fn queued(conn: &mut Conn, now: OffsetDateTime) -> Vec<ObjectKey> {
    conn.due_object_deletions(now, 100).await.unwrap()
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn crud_and_the_bytes_stored(pool: PgPool) {
    let mut conn = conn(&pool).await;
    let alice = user(&mut conn, "alice@example.com").await;
    let bob = user(&mut conn, "bob@example.com").await;
    assert_eq!(conn.stored_bytes(alice.id()).await.unwrap(), 0);

    let created = file(&mut conn, &alice, "report.pdf", 1_000).await;
    file(&mut conn, &alice, "notes.txt", 24).await;
    file(&mut conn, &bob, "other.txt", 7).await;
    assert_eq!(created.size().bytes(), 1_000);
    assert!(created.object_key().as_str().starts_with("files/"));
    assert_eq!(conn.stored_bytes(alice.id()).await.unwrap(), 1_024);
    assert_eq!(conn.stored_bytes(bob.id()).await.unwrap(), 7);

    let found = Repository::<StoredFile>::find_by_id(&mut conn, created.id())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found, created);

    let renamed = Repository::<StoredFile>::update(
        &mut conn,
        created.id(),
        &FileChanges {
            name: Some(FileName::parse("final.pdf").unwrap()),
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(renamed.name().as_str(), "final.pdf");
    assert_eq!(renamed.object_key(), created.object_key());
    assert!(renamed.updated_at() > created.updated_at());
    assert!(
        Repository::<StoredFile>::update(&mut conn, FileId::generate(), &FileChanges::default())
            .await
            .unwrap()
            .is_none()
    );

    let page = Repository::<StoredFile>::list(
        &mut conn,
        &FileFilter {
            owner_id: Some(alice.id()),
        },
        PageRequest::new(PageSize::parse(Some(1)).unwrap(), None),
    )
    .await
    .unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].name().as_str(), "notes.txt");
    let rest = Repository::<StoredFile>::list(
        &mut conn,
        &FileFilter {
            owner_id: Some(alice.id()),
        },
        PageRequest::new(PageSize::parse(Some(5)).unwrap(), page.next),
    )
    .await
    .unwrap();
    assert_eq!(rest.items.len(), 1);
    assert!(rest.next.is_none());
    let everyone = Repository::<StoredFile>::list(
        &mut conn,
        &FileFilter::default(),
        PageRequest::new(PageSize::parse(Some(10)).unwrap(), None),
    )
    .await
    .unwrap();
    assert_eq!(everyone.items.len(), 3);

    let locked = {
        let db = PostgresDatabase::new(pool.clone());
        let mut tx = db.transaction().await.unwrap();
        let locked = Repository::<StoredFile>::find_by_id_for_update(&mut tx, created.id())
            .await
            .unwrap();
        tx.commit().await.unwrap();
        locked
    };
    assert_eq!(locked.unwrap().id(), created.id());
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn a_deleted_file_queues_its_object(pool: PgPool) {
    let mut conn = conn(&pool).await;
    let alice = user(&mut conn, "alice@example.com").await;
    let stored = file(&mut conn, &alice, "a.txt", 1).await;
    let now = OffsetDateTime::now_utc();
    assert!(queued(&mut conn, now).await.is_empty());

    assert!(
        Repository::<StoredFile>::delete(&mut conn, stored.id())
            .await
            .unwrap()
    );
    assert!(
        !Repository::<StoredFile>::delete(&mut conn, stored.id())
            .await
            .unwrap()
    );

    assert_eq!(
        queued(&mut conn, now - Duration::days(365)).await,
        [stored.object_key().clone()]
    );
    assert!(
        conn.cancel_object_deletion(stored.object_key())
            .await
            .unwrap()
    );
    assert!(
        !conn
            .cancel_object_deletion(stored.object_key())
            .await
            .unwrap()
    );
    assert!(queued(&mut conn, now).await.is_empty());
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn deleting_an_account_queues_every_object_of_it(pool: PgPool) {
    let mut conn = conn(&pool).await;
    let alice = user(&mut conn, "alice@example.com").await;
    let bob = user(&mut conn, "bob@example.com").await;
    let mut keys: Vec<ObjectKey> = Vec::new();
    for name in ["1.txt", "2.txt", "3.txt"] {
        keys.push(file(&mut conn, &alice, name, 1).await.object_key().clone());
    }
    file(&mut conn, &bob, "kept.txt", 1).await;

    assert!(conn.delete_user(alice.id()).await.unwrap());

    let mut due = queued(&mut conn, OffsetDateTime::now_utc()).await;
    due.sort();
    keys.sort();
    assert_eq!(due, keys);
    assert_eq!(conn.stored_bytes(bob.id()).await.unwrap(), 1);
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn a_rolled_back_delete_queues_nothing(pool: PgPool) {
    let mut conn = conn(&pool).await;
    let alice = user(&mut conn, "alice@example.com").await;
    let stored = file(&mut conn, &alice, "a.txt", 1).await;

    {
        let db = PostgresDatabase::new(pool.clone());
        let mut tx = db.transaction().await.unwrap();
        Repository::<StoredFile>::delete(&mut tx, stored.id())
            .await
            .unwrap();
    }

    assert!(
        queued(&mut conn, OffsetDateTime::now_utc())
            .await
            .is_empty()
    );
    assert!(
        Repository::<StoredFile>::find_by_id(&mut conn, stored.id())
            .await
            .unwrap()
            .is_some()
    );
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn the_storage_lock_is_per_owner_and_held_until_the_transaction_ends(pool: PgPool) {
    // Must match `STORAGE_LOCK_SPACE` in the repository.
    const SPACE: i32 = 0x6669_6c65;
    let mut conn = conn(&pool).await;
    let alice = user(&mut conn, "alice@example.com").await;
    let bob = user(&mut conn, "bob@example.com").await;
    let try_lock = |owner: &User| {
        let pool = pool.clone();
        let owner = owner.id();
        async move {
            let mut probe = pool.acquire().await.unwrap();
            let acquired: bool = sqlx::query_scalar(
                "select pg_try_advisory_xact_lock($1, hashtext($2::uuid::text))",
            )
            .bind(SPACE)
            .bind(owner.as_uuid())
            .fetch_one(&mut *probe)
            .await
            .unwrap();
            acquired
        }
    };

    let db = PostgresDatabase::new(pool.clone());
    let mut tx = db.transaction().await.unwrap();
    tx.lock_storage(alice.id()).await.unwrap();
    assert!(!try_lock(&alice).await, "the lock should be held");
    assert!(try_lock(&bob).await, "other owners should not wait");

    tx.commit().await.unwrap();
    assert!(try_lock(&alice).await, "the lock should be released");
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn scheduled_deletions_wait_for_their_time(pool: PgPool) {
    let mut conn = conn(&pool).await;
    let now = OffsetDateTime::now_utc();
    let later = new_object_key();
    let sooner = new_object_key();
    let asap = new_object_key();

    conn.schedule_object_deletion(&later, Some(now + Duration::hours(2)))
        .await
        .unwrap();
    conn.schedule_object_deletion(&sooner, Some(now + Duration::hours(1)))
        .await
        .unwrap();
    conn.schedule_object_deletion(&asap, None).await.unwrap();

    assert_eq!(queued(&mut conn, now).await, std::slice::from_ref(&asap));
    assert_eq!(
        queued(&mut conn, now + Duration::hours(3)).await,
        [asap.clone(), sooner.clone(), later.clone()]
    );
    assert_eq!(
        conn.due_object_deletions(now + Duration::hours(3), 2)
            .await
            .unwrap(),
        [asap.clone(), sooner.clone()]
    );

    conn.schedule_object_deletion(&asap, Some(now + Duration::days(1)))
        .await
        .unwrap();
    assert!(queued(&mut conn, now).await.is_empty());
    conn.schedule_object_deletion(&later, None).await.unwrap();
    assert_eq!(queued(&mut conn, now).await, [later]);
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn a_file_for_a_deleted_owner_is_refused(pool: PgPool) {
    let mut conn = conn(&pool).await;
    let alice = user(&mut conn, "alice@example.com").await;
    conn.delete_user(alice.id()).await.unwrap();

    let err = Repository::<StoredFile>::create(
        &mut conn,
        FileId::generate(),
        &NewFile {
            owner_id: alice.id(),
            name: FileName::parse("late.txt").unwrap(),
            content_type: ContentType::parse("").unwrap(),
            size: FileSize::new(0).unwrap(),
            object_key: new_object_key(),
        },
    )
    .await
    .unwrap_err();

    assert!(matches!(
        err,
        domain::error::StorageError::ForeignKeyViolation { .. }
    ));
}
