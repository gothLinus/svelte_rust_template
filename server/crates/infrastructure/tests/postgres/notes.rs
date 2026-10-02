use domain::{
    note::{NewNote, Note, NoteBody, NoteChanges, NoteFilter, NoteId, NoteTitle},
    pagination::{PageRequest, PageSize},
    repository::{Repository, Version, Versioned},
    user::User,
};
use sqlx::PgPool;

use crate::support::{Conn, conn, user};

async fn note(conn: &mut Conn, owner: &User, title: &str) -> Note {
    Repository::<Note>::create(
        conn,
        domain::note::NoteId::generate(),
        &NewNote {
            owner_id: owner.id(),
            title: NoteTitle::parse(title).unwrap(),
            body: NoteBody::parse("body").unwrap(),
        },
    )
    .await
    .unwrap()
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn crud(pool: PgPool) {
    let mut conn = conn(&pool).await;
    let alice = user(&mut conn, "alice@example.com").await;

    let created = note(&mut conn, &alice, "Groceries").await;
    assert_eq!(created.owner_id(), alice.id());
    assert_eq!(created.created_at(), created.updated_at());

    let found = Repository::<Note>::find_by_id(&mut conn, created.id())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(found, created);

    let updated = Repository::<Note>::update(
        &mut conn,
        created.id(),
        &NoteChanges {
            title: None,
            body: Some(NoteBody::parse("milk").unwrap()),
        },
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(updated.title().as_str(), "Groceries");
    assert_eq!(updated.body().as_str(), "milk");
    assert!(updated.updated_at() > created.updated_at());
    assert_eq!(created.version(), Version::FIRST);
    assert_eq!(updated.version(), Version::FIRST.next());

    let unchanged = Repository::<Note>::update(&mut conn, created.id(), &NoteChanges::default())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(unchanged.updated_at(), updated.updated_at());
    assert_eq!(unchanged.version(), updated.version());

    assert!(
        Repository::<Note>::delete(&mut conn, created.id())
            .await
            .unwrap()
    );
    assert!(
        !Repository::<Note>::delete(&mut conn, created.id())
            .await
            .unwrap()
    );
    assert!(
        Repository::<Note>::update(&mut conn, created.id(), &NoteChanges::default())
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        Repository::<Note>::find_by_id(&mut conn, NoteId::generate())
            .await
            .unwrap()
            .is_none()
    );
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn list_filters_by_owner_and_pages(pool: PgPool) {
    let mut conn = conn(&pool).await;
    let alice = user(&mut conn, "alice@example.com").await;
    let bob = user(&mut conn, "bob@example.com").await;
    for n in 1..=3 {
        note(&mut conn, &alice, &format!("alice {n}")).await;
    }
    note(&mut conn, &bob, "bob 1").await;
    let size = PageSize::parse(Some(2)).unwrap();
    let alices = NoteFilter {
        owner_id: Some(alice.id()),
    };

    let first = Repository::<Note>::list(&mut conn, &alices, PageRequest::new(size, None))
        .await
        .unwrap();
    let titles: Vec<&str> = first.items.iter().map(|n| n.title().as_str()).collect();
    assert_eq!(titles, ["alice 3", "alice 2"]);

    let second = Repository::<Note>::list(&mut conn, &alices, PageRequest::new(size, first.next))
        .await
        .unwrap();
    let titles: Vec<&str> = second.items.iter().map(|n| n.title().as_str()).collect();
    assert_eq!(titles, ["alice 1"]);
    assert!(second.next.is_none());

    let everyone =
        Repository::<Note>::list(&mut conn, &NoteFilter::default(), PageRequest::default())
            .await
            .unwrap();
    assert_eq!(everyone.items.len(), 4);
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn notes_are_deleted_with_their_owner(pool: PgPool) {
    use domain::user::UserRepository;

    let mut conn = conn(&pool).await;
    let alice = user(&mut conn, "alice@example.com").await;
    let created = note(&mut conn, &alice, "mine").await;

    conn.delete_user(alice.id()).await.unwrap();

    assert!(
        Repository::<Note>::find_by_id(&mut conn, created.id())
            .await
            .unwrap()
            .is_none()
    );
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn a_locking_read_holds_the_row_until_the_transaction_ends(pool: PgPool) {
    use domain::database::{Database, Transaction};
    use infrastructure::db::PostgresDatabase;

    let mut conn = conn(&pool).await;
    let alice = user(&mut conn, "alice@example.com").await;
    let created = note(&mut conn, &alice, "Locked").await;
    drop(conn);

    let db = PostgresDatabase::new(pool.clone());
    let mut tx = db.transaction().await.unwrap();
    let locked = Repository::<Note>::find_by_id_for_update(&mut tx, created.id())
        .await
        .unwrap();
    assert_eq!(locked, Some(created.clone()));

    let contended = sqlx::query("select id from notes where id = $1 for update nowait")
        .bind(created.id().as_uuid())
        .execute(&pool)
        .await;
    assert!(contended.is_err(), "the row should be locked");

    tx.commit().await.unwrap();
    sqlx::query("select id from notes where id = $1 for update nowait")
        .bind(created.id().as_uuid())
        .execute(&pool)
        .await
        .unwrap();
    assert!(
        Repository::<Note>::find_by_id_for_update(
            &mut db.connection().await.unwrap(),
            NoteId::generate()
        )
        .await
        .unwrap()
        .is_none()
    );
}
