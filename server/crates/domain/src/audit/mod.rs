//! The audit log: an append-only record of what happened to an account's security, for its owner
//! and for administrators.
//!
//! An [`AuditEvent`] names its subject (the account it is about), the [`AuditAction`], who did it
//! when that was someone else (an administrator) and the client it came from. Events are written in
//! the same unit of work as the change they record, so a change and its record commit or roll back
//! together. They are kept for a configurable time and deleted with their account. Storage is
//! [`AuditRepository`].

use std::fmt::{self, Display, Formatter};

use time::OffsetDateTime;

use crate::{error::UnknownValue, id::Id, session::ClientInfo, user::UserId};

pub use repository::AuditRepository;

mod repository;

pub type AuditEventId = Id<AuditEvent>;

/// The longest [`AuditEvent::detail`] kept; longer ones are cut. It holds names (a role, a
/// provider, a passkey's label), never free text from a request body.
pub const MAX_AUDIT_DETAIL_LEN: usize = 200;

macro_rules! audit_actions {
    ($($variant:ident => $name:literal;)*) => {
        /// What happened. The stable name is stored and sent to clients, which word it themselves.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum AuditAction {
            $($variant,)*
        }

        impl AuditAction {
            pub const ALL: [Self; [$($name),*].len()] = [$(Self::$variant),*];

            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $name,)*
                }
            }
        }
    };
}

audit_actions! {
    Registered => "registered";
    SignedIn => "signed_in";
    SignInFailed => "sign_in_failed";
    SignedOutEverywhere => "signed_out_everywhere";
    SessionRevoked => "session_revoked";
    Reauthenticated => "reauthenticated";
    EmailVerified => "email_verified";
    EmailChanged => "email_changed";
    EmailChangeCancelled => "email_change_cancelled";
    PasswordReset => "password_reset";
    UsernameChanged => "username_changed";
    PhoneAdded => "phone_added";
    PhoneRemoved => "phone_removed";
    TotpAdded => "totp_added";
    TotpRemoved => "totp_removed";
    RecoveryCodesRegenerated => "recovery_codes_regenerated";
    PasskeyAdded => "passkey_added";
    PasskeyRemoved => "passkey_removed";
    IdentityLinked => "identity_linked";
    IdentityUnlinked => "identity_unlinked";
    RoleGranted => "role_granted";
    RoleRevoked => "role_revoked";
    AccountDisabled => "account_disabled";
    AccountEnabled => "account_enabled";
}

impl AuditAction {
    pub fn parse(raw: &str) -> Result<Self, UnknownValue> {
        Self::ALL
            .into_iter()
            .find(|action| action.as_str() == raw)
            .ok_or_else(|| UnknownValue::new("audit action", raw))
    }
}

impl Display for AuditAction {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// How a sign-in or re-authentication was proven, kept as an event's detail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMethod<'a> {
    Password,
    Registration,
    MagicLink,
    EmailCode,
    PhoneCode,
    Passkey,
    Totp,
    RecoveryCode,
    /// A social account, by provider id (`google`).
    Provider(&'a str),
}

impl AuthMethod<'_> {
    pub fn detail(self) -> String {
        match self {
            Self::Password => "password".to_owned(),
            Self::Registration => "registration".to_owned(),
            Self::MagicLink => "magic_link".to_owned(),
            Self::EmailCode => "email_code".to_owned(),
            Self::PhoneCode => "phone_code".to_owned(),
            Self::Passkey => "passkey".to_owned(),
            Self::Totp => "totp".to_owned(),
            Self::RecoveryCode => "recovery_code".to_owned(),
            Self::Provider(provider) => format!("provider:{provider}"),
        }
    }
}

/// A recorded event.
#[derive(Debug, Clone)]
pub struct AuditEvent {
    pub id: AuditEventId,
    /// The account the event is about.
    pub user_id: UserId,
    /// Who caused it: the user, an administrator, or `None` when nobody was signed in (a sign-in, a
    /// link from a mail). It stays when that account is deleted, so the event still shows that
    /// someone else acted.
    pub actor_id: Option<UserId>,
    pub action: AuditAction,
    pub detail: Option<String>,
    pub client: ClientInfo,
    pub occurred_at: OffsetDateTime,
}

/// An event to record. `occurred_at` comes from the injected clock, so retention is testable.
#[derive(Debug, Clone)]
pub struct NewAuditEvent {
    pub id: AuditEventId,
    pub user_id: UserId,
    pub actor_id: Option<UserId>,
    pub action: AuditAction,
    pub detail: Option<String>,
    pub client: ClientInfo,
    pub occurred_at: OffsetDateTime,
}

impl NewAuditEvent {
    pub fn new(
        user_id: UserId,
        action: AuditAction,
        client: ClientInfo,
        occurred_at: OffsetDateTime,
    ) -> Self {
        Self {
            id: AuditEventId::generate_at(occurred_at),
            user_id,
            actor_id: None,
            action,
            detail: None,
            client,
            occurred_at,
        }
    }

    #[must_use]
    pub fn by(mut self, actor: UserId) -> Self {
        self.actor_id = Some(actor);
        self
    }

    /// Sets the detail, cut to [`MAX_AUDIT_DETAIL_LEN`] characters.
    #[must_use]
    pub fn detail(mut self, detail: impl Into<String>) -> Self {
        let detail: String = detail.into();
        self.detail = Some(detail.chars().take(MAX_AUDIT_DETAIL_LEN).collect());
        self
    }
}

/// Which events a listing returns. Without a user, every account's.
#[derive(Debug, Clone, Copy, Default)]
pub struct AuditFilter {
    pub user_id: Option<UserId>,
}
