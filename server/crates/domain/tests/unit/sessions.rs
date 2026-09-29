use std::net::{IpAddr, Ipv4Addr};

use domain::{
    secret::TokenHash,
    session::{ClientInfo, MAX_USER_AGENT_LEN, Session, SessionPolicy},
    user::UserId,
};
use time::{Duration, OffsetDateTime, macros::datetime};

const START: OffsetDateTime = datetime!(2026-01-01 12:00 UTC);

fn policy() -> SessionPolicy {
    SessionPolicy {
        idle_timeout: Duration::hours(2),
        absolute_lifetime: Duration::hours(10),
        touch_interval: Duration::minutes(1),
    }
}

fn session() -> Session {
    Session::start(
        UserId::generate(),
        TokenHash::new([1; 32]),
        ClientInfo::default(),
        START,
        &policy(),
    )
}

#[test]
fn a_new_session_is_active() {
    let session = session();
    assert!(session.is_active(START, &policy()));
    assert_eq!(session.created_at(), START);
    assert_eq!(session.last_seen_at(), START);
    assert_eq!(session.expires_at(), START + Duration::hours(10));
    assert!(!session.rotation_pending());
}

#[test]
fn a_session_times_out_when_idle() {
    let session = session();
    assert!(session.is_active(START + Duration::hours(2) - Duration::seconds(1), &policy()));
    assert!(!session.is_active(START + Duration::hours(2), &policy()));
}

#[test]
fn activity_slides_the_idle_timeout_but_not_the_absolute_expiry() {
    let mut session = session();
    let mut now = START;
    for _ in 0..8 {
        now += Duration::hours(1);
        assert!(session.is_active(now, &policy()));
        session.touch(now);
    }
    // 9h after sign-in, last seen at 8h: idle expiry would be 10h, capped at 10h.
    assert_eq!(
        session.idle_expires_at(&policy()),
        START + Duration::hours(10)
    );
    session.touch(START + Duration::hours(9));
    assert!(!session.is_active(START + Duration::hours(10), &policy()));
}

#[test]
fn touches_are_throttled() {
    let session = session();
    assert!(!session.needs_touch(START + Duration::seconds(59), &policy()));
    assert!(session.needs_touch(START + Duration::seconds(60), &policy()));
}

#[test]
fn rotation_replaces_the_token_and_keeps_the_expiry() {
    let mut session = session();
    let expires_at = session.expires_at();

    session.rotate(TokenHash::new([2; 32]), START + Duration::hours(1));

    assert_eq!(session.token_hash(), &TokenHash::new([2; 32]));
    assert_eq!(session.last_seen_at(), START + Duration::hours(1));
    assert_eq!(session.expires_at(), expires_at);
    assert!(!session.rotation_pending());
}

#[test]
fn idle_cutoff() {
    assert_eq!(policy().idle_cutoff(START), START - Duration::hours(2));
}

#[test]
fn client_info_cleans_the_user_agent() {
    let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);

    assert_eq!(ClientInfo::new(Some(ip), Some("   ")).user_agent, None);
    assert_eq!(
        ClientInfo::new(None, Some(" curl/8 "))
            .user_agent
            .as_deref(),
        Some("curl/8")
    );
    let long = "x".repeat(MAX_USER_AGENT_LEN * 2);
    assert_eq!(
        ClientInfo::new(None, Some(&long))
            .user_agent
            .unwrap()
            .chars()
            .count(),
        MAX_USER_AGENT_LEN
    );
    assert_eq!(ClientInfo::new(Some(ip), None).ip, Some(ip));
}

#[test]
fn default_policy_is_sane() {
    let policy = SessionPolicy::default();
    assert!(policy.idle_timeout <= policy.absolute_lifetime);
    assert!(policy.touch_interval < policy.idle_timeout);
}

#[test]
fn a_session_is_recently_authenticated_for_exactly_the_window() {
    use domain::session::REAUTH_WINDOW;

    let mut session = session();
    assert!(session.is_recently_authenticated(START));
    assert!(session.is_recently_authenticated(START + REAUTH_WINDOW - Duration::seconds(1)));
    assert!(!session.is_recently_authenticated(START + REAUTH_WINDOW));

    let later = START + Duration::hours(3);
    assert!(!session.is_recently_authenticated(later));
    session.reauthenticate(later);
    assert!(session.is_recently_authenticated(later));
    assert!(!session.is_recently_authenticated(later + REAUTH_WINDOW));
}
