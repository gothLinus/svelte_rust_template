use domain::{
    error::StorageError,
    pagination::{PageRequest, PageSize},
    user::{
        EMAIL_UNIQUE_CONSTRAINT, Email, NewUser, PasswordHash, UserFilter, UserRepository, Username,
    },
};
use sqlx::PgPool;
use time::{Duration, OffsetDateTime};

use crate::support::{conn, new_user, user};

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn create_and_find(pool: PgPool) {
    let mut conn = conn(&pool).await;

    let created = conn
        .create_user(&new_user("alice@example.com"))
        .await
        .unwrap();

    assert_eq!(created.email().as_str(), "alice@example.com");
    assert!(!created.is_email_verified());
    assert!(!created.is_disabled());
    assert_eq!(created.id().as_uuid().get_version_num(), 7);

    let found = conn.find_user(created.id()).await.unwrap().unwrap();
    assert_eq!(found.id(), created.id());
    let by_email = conn
        .find_user_by_email(&Email::parse("ALICE@example.com").unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(by_email.id(), created.id());
    assert!(
        conn.find_user_for_update(created.id())
            .await
            .unwrap()
            .is_some()
    );
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn emails_are_unique_regardless_of_case(pool: PgPool) {
    let mut conn = conn(&pool).await;
    conn.create_user(&new_user("alice@example.com"))
        .await
        .unwrap();

    let err = conn
        .create_user(&NewUser {
            id: domain::user::UserId::generate(),
            username: Username::parse("alice2").unwrap(),
            ..new_user("alice@example.com")
        })
        .await
        .unwrap_err();
    assert!(err.is_unique_violation(EMAIL_UNIQUE_CONSTRAINT), "{err:?}");

    let err = sqlx::query(
        "insert into users (id, email, username, password_hash) \
         values (gen_random_uuid(), 'Alice@Example.com', 'alice3', 'x')",
    )
    .execute(&pool)
    .await
    .unwrap_err();
    assert!(err.to_string().contains(EMAIL_UNIQUE_CONSTRAINT));
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn list_pages_newest_first_and_searches(pool: PgPool) {
    let mut conn = conn(&pool).await;
    for name in ["alice", "bob", "carol", "dave"] {
        conn.create_user(&new_user(&format!("{name}@example.com")))
            .await
            .unwrap();
    }
    let size = PageSize::parse(Some(3)).unwrap();

    let first = conn
        .list_users(&UserFilter::default(), PageRequest::new(size, None))
        .await
        .unwrap();
    let emails: Vec<&str> = first.items.iter().map(|u| u.email().as_str()).collect();
    assert_eq!(
        emails,
        ["dave@example.com", "carol@example.com", "bob@example.com"]
    );

    let second = conn
        .list_users(&UserFilter::default(), PageRequest::new(size, first.next))
        .await
        .unwrap();
    assert_eq!(second.items.len(), 1);
    assert_eq!(second.items[0].email().as_str(), "alice@example.com");
    assert!(second.next.is_none());

    let search = |term: &str| UserFilter {
        search: Some(term.to_owned()),
    };
    let found = conn
        .list_users(&search("CAR"), PageRequest::default())
        .await
        .unwrap();
    assert_eq!(found.items.len(), 1);
    let none = conn
        .list_users(&search("%"), PageRequest::default())
        .await
        .unwrap();
    assert!(none.items.is_empty());
    conn.set_user_username(first.items[0].id(), &Username::parse("davey").unwrap())
        .await
        .unwrap();
    let by_username = conn
        .list_users(&search("DAVEY"), PageRequest::default())
        .await
        .unwrap();
    assert_eq!(by_username.items.len(), 1);
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn updates(pool: PgPool) {
    let mut conn = conn(&pool).await;
    let alice = user(&mut conn, "alice@example.com").await;
    let now = OffsetDateTime::now_utc().replace_nanosecond(0).unwrap();

    let renamed = conn
        .set_user_username(alice.id(), &Username::parse("Alice.L").unwrap())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(renamed.username().as_str(), "alice.l");
    assert!(renamed.updated_at() >= alice.updated_at());

    assert!(
        conn.set_user_password(alice.id(), Some(&PasswordHash::new("$argon2id$new")))
            .await
            .unwrap()
    );
    let reloaded = conn.find_user(alice.id()).await.unwrap().unwrap();
    assert_eq!(reloaded.password_hash().unwrap().as_str(), "$argon2id$new");

    let verified = conn
        .mark_user_email_verified(alice.id(), now)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(verified.email_verified_at(), Some(now));
    let again = conn
        .mark_user_email_verified(alice.id(), now + Duration::days(1))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(again.email_verified_at(), Some(now));

    let disabled = conn
        .set_user_disabled(alice.id(), Some(now))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(disabled.disabled_at(), Some(now));
    let still = conn
        .set_user_disabled(alice.id(), Some(now + Duration::days(1)))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(still.disabled_at(), Some(now));
    let enabled = conn
        .set_user_disabled(alice.id(), None)
        .await
        .unwrap()
        .unwrap();
    assert!(!enabled.is_disabled());
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn missing_users(pool: PgPool) {
    let mut conn = conn(&pool).await;
    let id = domain::user::UserId::generate();
    let username = Username::parse("nobody").unwrap();

    assert!(conn.find_user(id).await.unwrap().is_none());
    assert!(
        conn.set_user_username(id, &username)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        !conn
            .set_user_password(id, Some(&PasswordHash::new("x")))
            .await
            .unwrap()
    );
    assert!(!conn.delete_user(id).await.unwrap());
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn deleting_a_user_cascades(pool: PgPool) {
    let mut conn = conn(&pool).await;
    let alice = user(&mut conn, "alice@example.com").await;

    assert!(conn.delete_user(alice.id()).await.unwrap());

    let roles: i64 = sqlx::query_scalar("select count(*) from user_roles")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(roles, 0);
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn corrupt_rows_are_reported(pool: PgPool) {
    let mut conn = conn(&pool).await;
    let alice = user(&mut conn, "alice@example.com").await;
    sqlx::query("update users set email = 'not-an-email' where id = $1")
        .bind(alice.id().as_uuid())
        .execute(&pool)
        .await
        .unwrap();

    let err = conn.find_user(alice.id()).await.unwrap_err();
    assert!(matches!(err, StorageError::Corrupt(_)), "{err:?}");
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn unverified_accounts_past_the_cutoff_are_deleted_admins_never(pool: PgPool) {
    use domain::rbac::{RbacRepository, RoleName};

    let mut conn = conn(&pool).await;
    let stale = user(&mut conn, "stale@example.com").await;
    let verified = user(&mut conn, "verified@example.com").await;
    conn.mark_user_email_verified(verified.id(), OffsetDateTime::now_utc())
        .await
        .unwrap();
    let admin = user(&mut conn, "admin@example.com").await;
    conn.grant_role(admin.id(), &RoleName::ADMIN).await.unwrap();

    let an_hour_ago = OffsetDateTime::now_utc() - Duration::hours(1);
    assert_eq!(conn.delete_unverified_users(an_hour_ago).await.unwrap(), 0);

    let soon = OffsetDateTime::now_utc() + Duration::minutes(1);
    assert_eq!(conn.delete_unverified_users(soon).await.unwrap(), 1);
    assert!(conn.find_user(stale.id()).await.unwrap().is_none());
    assert!(conn.find_user(verified.id()).await.unwrap().is_some());
    assert!(conn.find_user(admin.id()).await.unwrap().is_some());
}
