//! Browser sessions behind opaque tokens.
//!
//! A [`Session`] is created by `application::auth` when a sign-in completes and loaded on every
//! authenticated request through [`SessionRepository`]. The cookie holds a random token and the
//! store only its digest ([`TokenHash`]). A session ends at whichever comes first of
//! [`SessionPolicy::idle_timeout`] after the last recorded request and
//! [`SessionPolicy::absolute_lifetime`] after sign-in. Expiry is decided here from the injected
//! clock; cleanup of expired rows is [`SessionRepository::delete_expired_sessions`].

use std::net::IpAddr;

use time::{Duration, OffsetDateTime};

use crate::{
    id::Id,
    rbac::PermissionSet,
    secret::TokenHash,
    user::{User, UserId},
};

pub use repository::SessionRepository;

mod repository;

pub type SessionId = Id<Session>;

pub const MAX_USER_AGENT_LEN: usize = 512;

/// How long after signing in or re-authenticating a session may make sensitive changes: adding or
/// removing a sign-in method, changing the address or number, deleting the account. After that
/// the user proves again who they are, so a stolen session alone cannot lock its owner out.
pub const REAUTH_WINDOW: Duration = Duration::minutes(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionPolicy {
    pub idle_timeout: Duration,
    pub absolute_lifetime: Duration,
    /// Activity is written back at most this often, so a busy session does not turn every request
    /// into a database write; the idle timeout is accurate to within this interval.
    pub touch_interval: Duration,
}

impl Default for SessionPolicy {
    fn default() -> Self {
        Self {
            idle_timeout: Duration::days(7),
            absolute_lifetime: Duration::days(30),
            touch_interval: Duration::minutes(1),
        }
    }
}

impl SessionPolicy {
    pub fn idle_cutoff(&self, now: OffsetDateTime) -> OffsetDateTime {
        now.saturating_sub(self.idle_timeout)
    }
}

/// Where a request came from, recorded on the session so users can recognize their devices in the
/// session list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClientInfo {
    pub ip: Option<IpAddr>,
    pub user_agent: Option<String>,
}

impl ClientInfo {
    /// Blank user agents are dropped and long ones truncated to [`MAX_USER_AGENT_LEN`] characters,
    /// because the header is client-controlled.
    pub fn new(ip: Option<IpAddr>, user_agent: Option<&str>) -> Self {
        let user_agent = user_agent
            .map(str::trim)
            .filter(|agent| !agent.is_empty())
            .map(|agent| agent.chars().take(MAX_USER_AGENT_LEN).collect());
        Self { ip, user_agent }
    }
}

/// A signed-in browser. The cookie holds a random token; only its digest is stored.
#[derive(Debug, Clone)]
pub struct Session {
    id: SessionId,
    user_id: UserId,
    token_hash: TokenHash,
    client: ClientInfo,
    created_at: OffsetDateTime,
    last_seen_at: OffsetDateTime,
    expires_at: OffsetDateTime,
    rotation_pending: bool,
    reauthenticated_at: OffsetDateTime,
}

pub struct SessionParts {
    pub id: SessionId,
    pub user_id: UserId,
    pub token_hash: TokenHash,
    pub client: ClientInfo,
    pub created_at: OffsetDateTime,
    pub last_seen_at: OffsetDateTime,
    pub expires_at: OffsetDateTime,
    pub rotation_pending: bool,
    pub reauthenticated_at: OffsetDateTime,
}

impl Session {
    pub fn start(
        user_id: UserId,
        token_hash: TokenHash,
        client: ClientInfo,
        now: OffsetDateTime,
        policy: &SessionPolicy,
    ) -> Self {
        Self {
            id: SessionId::generate_at(now),
            user_id,
            token_hash,
            client,
            created_at: now,
            last_seen_at: now,
            expires_at: now.saturating_add(policy.absolute_lifetime),
            rotation_pending: false,
            reauthenticated_at: now,
        }
    }

    pub fn from_parts(parts: SessionParts) -> Self {
        Self {
            id: parts.id,
            user_id: parts.user_id,
            token_hash: parts.token_hash,
            client: parts.client,
            created_at: parts.created_at,
            last_seen_at: parts.last_seen_at,
            expires_at: parts.expires_at,
            rotation_pending: parts.rotation_pending,
            reauthenticated_at: parts.reauthenticated_at,
        }
    }

    pub fn id(&self) -> SessionId {
        self.id
    }

    pub fn user_id(&self) -> UserId {
        self.user_id
    }

    pub fn token_hash(&self) -> &TokenHash {
        &self.token_hash
    }

    pub fn client(&self) -> &ClientInfo {
        &self.client
    }

    pub fn created_at(&self) -> OffsetDateTime {
        self.created_at
    }

    pub fn last_seen_at(&self) -> OffsetDateTime {
        self.last_seen_at
    }

    pub fn expires_at(&self) -> OffsetDateTime {
        self.expires_at
    }

    /// Whether the user's privileges changed since the token was issued, so the token must be
    /// replaced on the next request.
    pub fn rotation_pending(&self) -> bool {
        self.rotation_pending
    }

    pub fn reauthenticated_at(&self) -> OffsetDateTime {
        self.reauthenticated_at
    }

    pub fn is_recently_authenticated(&self, now: OffsetDateTime) -> bool {
        now < self.reauthenticated_at.saturating_add(REAUTH_WINDOW)
    }

    pub fn reauthenticate(&mut self, now: OffsetDateTime) {
        self.reauthenticated_at = now;
    }

    /// When the session ends if it sees no further requests: the idle timeout, capped by the
    /// absolute expiry.
    pub fn idle_expires_at(&self, policy: &SessionPolicy) -> OffsetDateTime {
        self.last_seen_at
            .saturating_add(policy.idle_timeout)
            .min(self.expires_at)
    }

    pub fn is_active(&self, now: OffsetDateTime, policy: &SessionPolicy) -> bool {
        now < self.idle_expires_at(policy)
    }

    pub fn needs_touch(&self, now: OffsetDateTime, policy: &SessionPolicy) -> bool {
        now - self.last_seen_at >= policy.touch_interval
    }

    pub fn touch(&mut self, now: OffsetDateTime) {
        self.last_seen_at = now;
    }

    /// Replaces the token after a password or privilege change. The old cookie stops working; the
    /// absolute expiry stays where it was.
    pub fn rotate(&mut self, token_hash: TokenHash, now: OffsetDateTime) {
        self.token_hash = token_hash;
        self.last_seen_at = now;
        self.rotation_pending = false;
    }
}

/// A session together with its user and their effective permissions, as loaded for every
/// authenticated request in a single query.
#[derive(Debug, Clone)]
pub struct AuthenticatedSession {
    pub session: Session,
    pub user: User,
    pub permissions: PermissionSet,
}
