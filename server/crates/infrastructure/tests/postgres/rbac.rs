use std::collections::BTreeSet;

use domain::{
    error::StorageError,
    rbac::{Permission, PermissionSet, RbacRepository, RoleName, default_user_permissions},
    user::UserRepository,
};
use sqlx::PgPool;

use crate::support::{conn, user};

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn permissions_in_the_database_match_the_code(pool: PgPool) {
    let in_database: BTreeSet<String> = sqlx::query_scalar("select name from permissions")
        .fetch_all(&pool)
        .await
        .unwrap()
        .into_iter()
        .collect();
    let in_code: BTreeSet<String> = Permission::ALL
        .iter()
        .map(|permission| permission.as_str().to_owned())
        .collect();

    assert_eq!(in_database, in_code);
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn seeded_roles(pool: PgPool) {
    let mut conn = conn(&pool).await;

    let roles = conn.list_roles().await.unwrap();

    let names: Vec<&str> = roles.iter().map(|role| role.name.as_str()).collect();
    assert_eq!(names, ["admin", "user"]);
    assert_eq!(roles[0].permissions, PermissionSet::all());
    assert_eq!(roles[1].permissions, default_user_permissions());
    let admin = conn.find_role(&RoleName::ADMIN).await.unwrap().unwrap();
    assert!(!admin.description.is_empty());
    assert!(
        conn.find_role(&RoleName::parse("nope").unwrap())
            .await
            .unwrap()
            .is_none()
    );
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn granting_and_revoking(pool: PgPool) {
    let mut conn = conn(&pool).await;
    let alice = user(&mut conn, "alice@example.com").await;
    let bob = user(&mut conn, "bob@example.com").await;

    assert!(conn.grant_role(alice.id(), &RoleName::ADMIN).await.unwrap());
    assert!(!conn.grant_role(alice.id(), &RoleName::ADMIN).await.unwrap());

    assert_eq!(
        conn.permissions_of_user(alice.id()).await.unwrap(),
        PermissionSet::all()
    );
    let roles = conn.roles_of_users(&[alice.id(), bob.id()]).await.unwrap();
    assert_eq!(roles.len(), 3);
    assert!(roles.contains(&(alice.id(), RoleName::ADMIN)));
    assert!(roles.contains(&(bob.id(), RoleName::USER)));

    assert!(
        conn.revoke_role(alice.id(), &RoleName::ADMIN)
            .await
            .unwrap()
    );
    assert!(
        !conn
            .revoke_role(alice.id(), &RoleName::ADMIN)
            .await
            .unwrap()
    );
    assert!(
        !conn
            .permissions_of_user(alice.id())
            .await
            .unwrap()
            .contains(Permission::UsersManage)
    );
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn granting_an_unknown_role_is_a_foreign_key_violation(pool: PgPool) {
    let mut conn = conn(&pool).await;
    let alice = user(&mut conn, "alice@example.com").await;

    let err = conn
        .grant_role(alice.id(), &RoleName::parse("ghost").unwrap())
        .await
        .unwrap_err();

    assert!(
        matches!(&err, StorageError::ForeignKeyViolation { constraint } if constraint == "user_roles_role_fkey"),
        "{err:?}"
    );
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn counting_enabled_holders_of_a_permission(pool: PgPool) {
    let mut conn = conn(&pool).await;
    let alice = user(&mut conn, "alice@example.com").await;
    let bob = user(&mut conn, "bob@example.com").await;
    conn.grant_role(alice.id(), &RoleName::ADMIN).await.unwrap();
    conn.grant_role(bob.id(), &RoleName::ADMIN).await.unwrap();

    assert_eq!(
        conn.count_enabled_users_with(Permission::UsersManage)
            .await
            .unwrap(),
        2
    );
    assert_eq!(
        conn.count_enabled_users_with(Permission::NotesRead)
            .await
            .unwrap(),
        2
    );

    conn.set_user_disabled(bob.id(), Some(time::OffsetDateTime::now_utc()))
        .await
        .unwrap();
    assert_eq!(
        conn.count_enabled_users_with(Permission::UsersManage)
            .await
            .unwrap(),
        1
    );
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn unknown_permissions_in_the_database_are_ignored(pool: PgPool) {
    let mut conn = conn(&pool).await;
    let alice = user(&mut conn, "alice@example.com").await;
    sqlx::query("insert into permissions (name) values ('future:thing')")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("insert into role_permissions (role, permission) values ('user', 'future:thing')")
        .execute(&pool)
        .await
        .unwrap();

    let permissions = conn.permissions_of_user(alice.id()).await.unwrap();

    assert_eq!(permissions, default_user_permissions());
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn the_role_assignment_lock_is_held_until_the_transaction_ends(pool: PgPool) {
    use domain::database::{Database, Transaction};

    // Must match `ROLE_ASSIGNMENT_LOCK` in the repository.
    const KEY: i64 = 0x726f_6c65_7300;
    let try_lock = || async {
        let mut probe = pool.acquire().await.unwrap();
        let acquired: bool = sqlx::query_scalar("select pg_try_advisory_xact_lock($1)")
            .bind(KEY)
            .fetch_one(&mut *probe)
            .await
            .unwrap();
        acquired
    };

    let db = infrastructure::db::PostgresDatabase::new(pool.clone());
    let mut tx = db.transaction().await.unwrap();
    tx.lock_role_assignments().await.unwrap();
    assert!(!try_lock().await, "the lock should be held");

    tx.commit().await.unwrap();
    assert!(try_lock().await, "the lock should be released");
}
