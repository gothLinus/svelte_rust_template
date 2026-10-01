use std::net::{IpAddr, Ipv4Addr};

use application::{
    audit::dto::{AuditEventDto, ListActivityQuery, ListAuditQuery},
    auth::dto::LoginRequest,
};
use domain::{session::ClientInfo, user::UserId};
use time::Duration;

use crate::support::{Fixture, Outcome, PASSWORD, secret};

const IP: IpAddr = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7));

fn from_ip() -> ClientInfo {
    ClientInfo::new(Some(IP), Some("Firefox"))
}

fn login(identifier: &str, password: &str) -> LoginRequest {
    LoginRequest {
        identifier: identifier.to_owned(),
        password: secret(password),
    }
}

fn actions(events: &[AuditEventDto]) -> Vec<&str> {
    events.iter().map(|event| event.action.as_str()).collect()
}

async fn activity(fx: &Fixture, actor: &application::actor::Actor) -> Vec<AuditEventDto> {
    fx.services
        .audit
        .activity(actor, ListActivityQuery::default())
        .await
        .unwrap()
        .items
}

#[tokio::test]
async fn registering_and_signing_in_are_recorded_with_the_method_and_the_client() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;

    fx.services
        .auth
        .login(login("alice@example.com", PASSWORD), from_ip(), None)
        .await
        .unwrap()
        .signed_in();

    let events = activity(&fx, &alice.actor).await;
    assert_eq!(actions(&events), ["signed_in", "signed_in", "registered"]);
    assert_eq!(events[0].detail.as_deref(), Some("password"));
    assert_eq!(events[0].ip.as_deref(), Some("203.0.113.7"));
    assert_eq!(events[0].user_agent.as_deref(), Some("Firefox"));
    assert_eq!(events[1].detail.as_deref(), Some("registration"));
    assert!(
        events
            .iter()
            .all(|event| !event.by_other && event.user.is_none())
    );
}

#[tokio::test]
async fn a_wrong_password_is_recorded_for_an_existing_account_only() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;

    for identifier in ["alice@example.com", "nobody@example.com"] {
        let err = fx
            .services
            .auth
            .login(login(identifier, "not the password"), from_ip(), None)
            .await
            .unwrap_err();
        assert_eq!(err.code(), "invalid_credentials");
    }

    let events = activity(&fx, &alice.actor).await;
    assert_eq!(events[0].action, "sign_in_failed");
    assert_eq!(events[0].detail.as_deref(), Some("password"));
    assert_eq!(events[0].ip.as_deref(), Some("203.0.113.7"));
    assert_eq!(fx.db.with(|state| state.audit_events.len()), 3);
}

#[tokio::test]
async fn a_wrong_second_step_is_recorded_and_the_right_one_signs_in() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    fx.enable_totp(&alice.actor).await;

    let pending = fx
        .services
        .auth
        .login(login("alice@example.com", PASSWORD), from_ip(), None)
        .await
        .unwrap()
        .mfa_required();
    let err = fx
        .services
        .mfa
        .complete_with_totp(
            &pending.token,
            application::mfa::dto::CodeRequest {
                code: secret("000000"),
            },
            from_ip(),
            None,
        )
        .await
        .unwrap_err();
    crate::support::assert_invalid_code(&err);
    fx.clock.advance(Duration::seconds(30));
    let code = fx.totp_now(alice.actor.user_id);
    fx.services
        .mfa
        .complete_with_totp(
            &pending.token,
            application::mfa::dto::CodeRequest {
                code: secret(&code),
            },
            from_ip(),
            None,
        )
        .await
        .unwrap();

    let events = activity(&fx, &alice.actor).await;
    assert_eq!(
        actions(&events[..3]),
        ["signed_in", "sign_in_failed", "totp_added"]
    );
    assert_eq!(events[0].detail.as_deref(), Some("totp"));
    assert_eq!(events[1].detail.as_deref(), Some("totp"));
}

#[tokio::test]
async fn an_administrators_change_names_them_in_the_log_but_not_to_the_owner() {
    let fx = Fixture::new();
    let admin = fx.admin("admin@example.com").await;
    let alice = fx.user("alice@example.com").await;

    fx.services
        .admin
        .grant_role(&admin.actor, alice.actor.user_id, "admin")
        .await
        .unwrap();

    let own = activity(&fx, &alice.actor).await;
    assert_eq!(own[0].action, "role_granted");
    assert_eq!(own[0].detail.as_deref(), Some("admin"));
    assert!(own[0].by_other);
    assert!(own[0].actor.is_none());

    let log = fx
        .services
        .audit
        .list(
            &admin.actor,
            ListAuditQuery {
                user: Some(alice.actor.user_id.to_string()),
                ..ListAuditQuery::default()
            },
        )
        .await
        .unwrap()
        .items;
    assert_eq!(log[0].action, "role_granted");
    assert_eq!(
        log[0].user.as_ref().map(|user| user.email.as_str()),
        Some("alice@example.com")
    );
    assert_eq!(
        log[0].actor.as_ref().map(|actor| actor.email.as_str()),
        Some("admin@example.com")
    );
    assert!(log.iter().all(
        |event| event.user.as_ref().map(|user| user.id) == Some(alice.actor.user_id.as_uuid())
    ));
}

#[tokio::test]
async fn a_change_that_rolls_back_leaves_no_event() {
    let fx = Fixture::new();
    let admin = fx.admin("admin@example.com").await;

    let err = fx
        .services
        .admin
        .revoke_role(&admin.actor, admin.actor.user_id, "admin")
        .await
        .unwrap_err();

    assert_eq!(err.code(), "last_admin");
    let own = activity(&fx, &admin.actor).await;
    assert!(!actions(&own).contains(&"role_revoked"));
}

#[tokio::test]
async fn the_audit_log_needs_audit_read_and_pages_every_account() {
    let fx = Fixture::new();
    let admin = fx.admin("admin@example.com").await;
    let alice = fx.user("alice@example.com").await;

    let err = fx
        .services
        .audit
        .list(&alice.actor, ListAuditQuery::default())
        .await
        .unwrap_err();
    assert_eq!(err.code(), "forbidden");

    let first = fx
        .services
        .audit
        .list(
            &admin.actor,
            ListAuditQuery {
                limit: Some(3),
                ..ListAuditQuery::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(first.items.len(), 3);
    let second = fx
        .services
        .audit
        .list(
            &admin.actor,
            ListAuditQuery {
                limit: Some(3),
                after: first.next_cursor,
                ..ListAuditQuery::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(second.items.len(), 1);
    assert!(second.next_cursor.is_none());
    let owners: Vec<&str> = first
        .items
        .iter()
        .chain(&second.items)
        .filter_map(|event| event.user.as_ref().map(|user| user.username.as_str()))
        .collect();
    assert_eq!(owners, ["alice", "alice", "admin", "admin"]);
}

#[tokio::test]
async fn the_user_filter_is_validated() {
    let fx = Fixture::new();
    let admin = fx.admin("admin@example.com").await;

    let err = fx
        .services
        .audit
        .list(
            &admin.actor,
            ListAuditQuery {
                user: Some("not-a-uuid".to_owned()),
                ..ListAuditQuery::default()
            },
        )
        .await
        .unwrap_err();

    let application::AppError::Validation(errors) = err else {
        panic!("expected a validation error, got {err:?}");
    };
    assert_eq!(errors.fields()[0].field, "user");
    assert_eq!(errors.fields()[0].code, "invalid_id");
}

#[tokio::test]
async fn security_changes_on_the_account_are_recorded() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    let other = fx
        .services
        .auth
        .login(login("alice@example.com", PASSWORD), from_ip(), None)
        .await
        .unwrap()
        .signed_in();

    fx.services
        .account
        .revoke_session(&alice.actor, other.session.id())
        .await
        .unwrap();
    fx.services
        .mfa
        .regenerate_recovery_codes(&alice.actor)
        .await
        .unwrap_err();
    fx.enable_totp(&alice.actor).await;
    fx.services
        .mfa
        .regenerate_recovery_codes(&alice.actor)
        .await
        .unwrap();
    fx.services
        .auth
        .logout_everywhere(&alice.actor)
        .await
        .unwrap();

    let events = activity(&fx, &alice.actor).await;
    assert_eq!(
        actions(&events[..5]),
        [
            "signed_out_everywhere",
            "recovery_codes_regenerated",
            "totp_added",
            "session_revoked",
            "signed_in",
        ]
    );
}

#[tokio::test]
async fn the_same_username_again_is_not_a_change() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;

    for username in ["alice", "alicia"] {
        fx.services
            .account
            .update_profile(
                &alice.actor,
                application::account::dto::UpdateProfileRequest {
                    username: username.to_owned(),
                },
            )
            .await
            .unwrap();
    }

    let events = activity(&fx, &alice.actor).await;
    assert_eq!(
        actions(&events),
        ["username_changed", "signed_in", "registered"]
    );
    assert_eq!(events[0].detail.as_deref(), Some("alicia"));
}

#[tokio::test]
async fn events_older_than_the_retention_are_deleted() {
    let fx = Fixture::with(|settings| settings.audit_retention = Some(Duration::days(90)));
    let alice = fx.user("alice@example.com").await;
    fx.clock.advance(Duration::days(91));
    fx.services
        .auth
        .login(login("alice@example.com", PASSWORD), from_ip(), None)
        .await
        .unwrap();

    let cleanup = fx.services.maintenance.delete_expired().await.unwrap();

    assert_eq!(cleanup.audit_events, 2);
    assert_eq!(actions(&activity(&fx, &alice.actor).await), ["signed_in"]);
}

#[tokio::test]
async fn deleting_the_account_deletes_its_events_and_keeps_others_by_it_anonymous() {
    let fx = Fixture::new();
    let admin = fx.admin("admin@example.com").await;
    let second = fx.admin("second@example.com").await;
    let alice = fx.user("alice@example.com").await;
    fx.services
        .admin
        .grant_role(&admin.actor, alice.actor.user_id, "admin")
        .await
        .unwrap();

    fx.services
        .account
        .delete_account(
            &admin.actor,
            application::account::dto::DeleteAccountRequest { password: None },
        )
        .await
        .unwrap();

    let admin_id: UserId = admin.actor.user_id;
    assert!(fx.db.with(|state| {
        state
            .audit_events
            .iter()
            .all(|event| event.user_id != admin_id)
    }));
    let log = fx
        .services
        .audit
        .list(&second.actor, ListAuditQuery::default())
        .await
        .unwrap()
        .items;
    let granted = log
        .iter()
        .find(|event| event.action == "role_granted")
        .unwrap();
    assert!(granted.by_other);
    assert!(granted.actor.is_none());
}
