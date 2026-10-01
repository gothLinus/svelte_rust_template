use std::net::{IpAddr, Ipv4Addr};

use domain::{
    audit::{AuditAction, AuditFilter, AuditRepository, NewAuditEvent},
    clock::truncate_to_micros,
    pagination::{PageRequest, PageSize},
    session::ClientInfo,
    user::{User, UserRepository},
};
use sqlx::PgPool;
use time::{Duration, OffsetDateTime};

use crate::support::{Conn, conn, user};

fn now() -> OffsetDateTime {
    truncate_to_micros(OffsetDateTime::now_utc())
}

async fn record(conn: &mut Conn, user: &User, action: AuditAction, at: OffsetDateTime) {
    let client = ClientInfo::new(
        Some(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7))),
        Some("curl"),
    );
    conn.record_audit_event(&NewAuditEvent::new(user.id(), action, client, at).detail("password"))
        .await
        .unwrap();
}

fn first(size: u32) -> PageRequest {
    PageRequest::new(PageSize::parse(Some(size)).unwrap(), None)
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn events_round_trip_newest_first_per_user_and_in_pages(pool: PgPool) {
    let mut conn = conn(&pool).await;
    let alice = user(&mut conn, "alice@example.com").await;
    let bob = user(&mut conn, "bob@example.com").await;
    let at = now();
    record(&mut conn, &alice, AuditAction::Registered, at).await;
    record(&mut conn, &bob, AuditAction::Registered, at).await;
    record(&mut conn, &alice, AuditAction::SignedIn, at).await;
    record(&mut conn, &alice, AuditAction::PasskeyAdded, at).await;

    let filter = AuditFilter {
        user_id: Some(alice.id()),
    };
    let page = conn.list_audit_events(&filter, first(2)).await.unwrap();
    let actions: Vec<AuditAction> = page.items.iter().map(|event| event.action).collect();
    assert_eq!(actions, [AuditAction::PasskeyAdded, AuditAction::SignedIn]);
    let event = &page.items[0];
    assert_eq!(event.user_id, alice.id());
    assert_eq!(event.actor_id, None);
    assert_eq!(event.detail.as_deref(), Some("password"));
    assert_eq!(
        event.client.ip,
        Some(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7)))
    );
    assert_eq!(event.client.user_agent.as_deref(), Some("curl"));
    assert_eq!(event.occurred_at, at);

    let rest = conn
        .list_audit_events(
            &filter,
            PageRequest::new(PageSize::parse(Some(2)).unwrap(), page.next),
        )
        .await
        .unwrap();
    assert_eq!(rest.items.len(), 1);
    assert_eq!(rest.items[0].action, AuditAction::Registered);
    assert!(rest.next.is_none());

    let everyone = conn
        .list_audit_events(&AuditFilter::default(), first(10))
        .await
        .unwrap();
    assert_eq!(everyone.items.len(), 4);
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn retention_deletes_only_older_events(pool: PgPool) {
    let mut conn = conn(&pool).await;
    let alice = user(&mut conn, "alice@example.com").await;
    let at = now();
    record(
        &mut conn,
        &alice,
        AuditAction::SignedIn,
        at - Duration::days(100),
    )
    .await;
    record(&mut conn, &alice, AuditAction::SignedIn, at).await;

    let deleted = conn
        .delete_audit_events_before(at - Duration::days(90))
        .await
        .unwrap();

    assert_eq!(deleted, 1);
    let left = conn
        .list_audit_events(&AuditFilter::default(), first(10))
        .await
        .unwrap();
    assert_eq!(left.items.len(), 1);
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn an_account_takes_its_events_along_but_not_what_it_did_to_others(pool: PgPool) {
    let mut conn = conn(&pool).await;
    let admin = user(&mut conn, "admin@example.com").await;
    let alice = user(&mut conn, "alice@example.com").await;
    record(&mut conn, &admin, AuditAction::SignedIn, now()).await;
    conn.record_audit_event(
        &NewAuditEvent::new(
            alice.id(),
            AuditAction::RoleGranted,
            ClientInfo::default(),
            now(),
        )
        .by(admin.id())
        .detail("admin"),
    )
    .await
    .unwrap();

    assert!(conn.delete_user(admin.id()).await.unwrap());

    let left = conn
        .list_audit_events(&AuditFilter::default(), first(10))
        .await
        .unwrap();
    assert_eq!(left.items.len(), 1);
    assert_eq!(left.items[0].user_id, alice.id());
    assert_eq!(left.items[0].actor_id, Some(admin.id()));
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn find_users_returns_the_ones_that_exist(pool: PgPool) {
    let mut conn = conn(&pool).await;
    let alice = user(&mut conn, "alice@example.com").await;
    let bob = user(&mut conn, "bob@example.com").await;

    let mut found: Vec<String> = conn
        .find_users(&[alice.id(), bob.id(), domain::user::UserId::generate()])
        .await
        .unwrap()
        .iter()
        .map(|user| user.email().to_string())
        .collect();
    found.sort();

    assert_eq!(found, ["alice@example.com", "bob@example.com"]);
    assert!(conn.find_users(&[]).await.unwrap().is_empty());
}
