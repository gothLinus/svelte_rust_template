use std::net::{IpAddr, Ipv6Addr};

use domain::{
    clock::truncate_to_micros,
    rbac::{Permission, RbacRepository, RoleName},
    secret::TokenHash,
    session::{ClientInfo, Session, SessionPolicy, SessionRepository},
    user::User,
};
use sqlx::PgPool;
use time::{Duration, OffsetDateTime};

use crate::support::{Conn, conn, user};

fn now() -> OffsetDateTime {
    truncate_to_micros(OffsetDateTime::now_utc())
}

async fn session(conn: &mut Conn, user: &User, token: u8, at: OffsetDateTime) -> Session {
    let session = Session::start(
        user.id(),
        TokenHash::new([token; 32]),
        ClientInfo::new(Some(IpAddr::V6(Ipv6Addr::LOCALHOST)), Some("Mozilla/5.0")),
        at,
        &SessionPolicy::default(),
    );
    conn.create_session(&session).await.unwrap();
    session
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn find_by_token_loads_the_user_and_permissions(pool: PgPool) {
    let mut conn = conn(&pool).await;
    let alice = user(&mut conn, "alice@example.com").await;
    let created = session(&mut conn, &alice, 1, now()).await;

    let found = conn
        .find_session_by_token(&TokenHash::new([1; 32]))
        .await
        .unwrap()
        .unwrap();

    assert_eq!(found.session.id(), created.id());
    assert_eq!(found.session.client(), created.client());
    assert_eq!(found.session.created_at(), created.created_at());
    assert_eq!(found.session.expires_at(), created.expires_at());
    assert_eq!(found.user.id(), alice.id());
    assert!(found.permissions.contains(Permission::NotesWrite));
    assert!(!found.permissions.contains(Permission::UsersManage));

    assert!(
        conn.find_session_by_token(&TokenHash::new([9; 32]))
            .await
            .unwrap()
            .is_none()
    );
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn a_user_without_roles_has_no_permissions(pool: PgPool) {
    let mut conn = conn(&pool).await;
    let alice = user(&mut conn, "alice@example.com").await;
    conn.revoke_role(alice.id(), &RoleName::USER).await.unwrap();
    session(&mut conn, &alice, 1, now()).await;

    let found = conn
        .find_session_by_token(&TokenHash::new([1; 32]))
        .await
        .unwrap()
        .unwrap();

    assert!(found.permissions.is_empty());
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn touch_and_rotate(pool: PgPool) {
    let mut conn = conn(&pool).await;
    let alice = user(&mut conn, "alice@example.com").await;
    let start = now();
    let mut stored = session(&mut conn, &alice, 1, start).await;

    stored.touch(start + Duration::minutes(5));
    assert!(conn.touch_session(&stored).await.unwrap());

    assert_eq!(
        conn.flag_user_sessions_for_rotation(alice.id())
            .await
            .unwrap(),
        1
    );
    let flagged = conn
        .find_session_by_token(&TokenHash::new([1; 32]))
        .await
        .unwrap()
        .unwrap()
        .session;
    assert!(flagged.rotation_pending());
    assert_eq!(flagged.last_seen_at(), start + Duration::minutes(5));

    let mut rotated = flagged.clone();
    rotated.rotate(TokenHash::new([2; 32]), start + Duration::minutes(6));
    assert!(
        !conn
            .rotate_session(&rotated, &TokenHash::new([7; 32]))
            .await
            .unwrap()
    );
    assert!(
        conn.rotate_session(&rotated, &TokenHash::new([1; 32]))
            .await
            .unwrap()
    );

    assert!(
        conn.find_session_by_token(&TokenHash::new([1; 32]))
            .await
            .unwrap()
            .is_none()
    );
    let found = conn
        .find_session_by_token(&TokenHash::new([2; 32]))
        .await
        .unwrap()
        .unwrap()
        .session;
    assert!(!found.rotation_pending());
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn listing_and_deleting(pool: PgPool) {
    let mut conn = conn(&pool).await;
    let alice = user(&mut conn, "alice@example.com").await;
    let bob = user(&mut conn, "bob@example.com").await;
    let start = now();
    let older = session(&mut conn, &alice, 1, start).await;
    let newer = session(&mut conn, &alice, 2, start + Duration::minutes(1)).await;
    let third = session(&mut conn, &alice, 3, start + Duration::minutes(2)).await;
    let bobs = session(&mut conn, &bob, 4, start).await;

    let listed = conn.list_user_sessions(alice.id()).await.unwrap();
    let ids: Vec<_> = listed.iter().map(Session::id).collect();
    assert_eq!(ids, [third.id(), newer.id(), older.id()]);

    assert!(
        !conn
            .delete_user_session(alice.id(), bobs.id())
            .await
            .unwrap()
    );
    assert!(
        conn.delete_user_session(alice.id(), third.id())
            .await
            .unwrap()
    );

    assert_eq!(
        conn.delete_user_sessions(alice.id(), Some(newer.id()))
            .await
            .unwrap(),
        1
    );
    assert!(
        conn.delete_session_by_token(&TokenHash::new([2; 32]))
            .await
            .unwrap()
    );
    assert!(conn.delete_session(bobs.id()).await.unwrap());
    assert!(!conn.delete_session(bobs.id()).await.unwrap());
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn expired_sessions_are_cleaned_up(pool: PgPool) {
    let mut conn = conn(&pool).await;
    let alice = user(&mut conn, "alice@example.com").await;
    let policy = SessionPolicy::default();
    let now = now();
    session(&mut conn, &alice, 1, now - Duration::days(31)).await;
    session(&mut conn, &alice, 2, now - Duration::days(8)).await;
    session(&mut conn, &alice, 3, now - Duration::days(1)).await;

    let deleted = conn
        .delete_expired_sessions(now, policy.idle_cutoff(now))
        .await
        .unwrap();

    assert_eq!(deleted, 2);
    assert_eq!(conn.list_user_sessions(alice.id()).await.unwrap().len(), 1);
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn a_large_backlog_is_cleaned_up_in_batches(pool: PgPool) {
    let mut conn = conn(&pool).await;
    let alice = user(&mut conn, "alice@example.com").await;
    let now = now();
    sqlx::query(
        "insert into sessions
             (id, user_id, token_hash, last_seen_at, expires_at, reauthenticated_at)
         select gen_random_uuid(), $1, sha256(i::text::bytea), $2, $2, $2
         from generate_series(1, 12001) as i",
    )
    .bind(alice.id().as_uuid())
    .bind(now - Duration::days(40))
    .execute(&pool)
    .await
    .unwrap();
    session(&mut conn, &alice, 1, now).await;

    let deleted = conn
        .delete_expired_sessions(now, SessionPolicy::default().idle_cutoff(now))
        .await
        .unwrap();

    assert_eq!(deleted, 12001);
    assert_eq!(conn.list_user_sessions(alice.id()).await.unwrap().len(), 1);
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn reauthentication_is_recorded(pool: PgPool) {
    let mut conn = conn(&pool).await;
    let alice = user(&mut conn, "alice@example.com").await;
    let signed_in_at = now() - Duration::hours(1);
    let created = session(&mut conn, &alice, 1, signed_in_at).await;
    let find = async |conn: &mut Conn| {
        conn.find_session_by_token(&TokenHash::new([1; 32]))
            .await
            .unwrap()
            .unwrap()
            .session
    };
    assert_eq!(find(&mut conn).await.reauthenticated_at(), signed_in_at);
    assert!(!find(&mut conn).await.is_recently_authenticated(now()));

    let at = now();
    assert!(
        conn.mark_session_reauthenticated(created.id(), at)
            .await
            .unwrap()
    );
    let found = find(&mut conn).await;
    assert_eq!(found.reauthenticated_at(), at);
    assert!(found.is_recently_authenticated(at));
    assert_eq!(
        conn.list_user_sessions(alice.id()).await.unwrap()[0].reauthenticated_at(),
        at
    );
}
