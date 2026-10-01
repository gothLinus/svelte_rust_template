//! What use cases need from the outside world, bundled so every service carries a single type
//! parameter.

use std::sync::Arc;

use domain::{
    audit::{AuditAction, AuditRepository, NewAuditEvent},
    clock::Clock,
    database::Database,
    error::ValidationError,
    i18n::{Locale, Message, Translator},
    identity::{IdentityProviders, IdentityRepository},
    mail::Mailer,
    mfa::MfaRepository,
    note::Note,
    one_time_code::OneTimeCodeRepository,
    passkey::PasskeyRepository,
    rbac::RbacRepository,
    repository::Repository,
    security::{Crypto, PasswordHasher, TokenGenerator},
    session::{ClientInfo, SessionPolicy, SessionRepository},
    text::TextSender,
    user::{UserId, UserRepository},
    user_token::{TokenPolicy, UserTokenRepository},
};

use crate::{
    actor::Actor,
    mail::{Links, Voice},
};

/// Every repository the application uses, implemented on one connection type.
///
/// Both halves of an [`Adapters::Db`], its connections and its transactions, are `Store`s, so a
/// use case's code runs unchanged with or without a surrounding transaction. Any type with all
/// the repository impls is a `Store` automatically.
///
/// When you add a resource, add its `Repository<Thing>` bound here, in the trait and in the
/// blanket impl; the compiler then points at the adapter that is missing it.
pub trait Store:
    UserRepository
    + RbacRepository
    + SessionRepository
    + UserTokenRepository
    + OneTimeCodeRepository
    + IdentityRepository
    + PasskeyRepository
    + MfaRepository
    + AuditRepository
    + Repository<Note>
{
}

impl<T> Store for T where
    T: UserRepository
        + RbacRepository
        + SessionRepository
        + UserTokenRepository
        + OneTimeCodeRepository
        + IdentityRepository
        + PasskeyRepository
        + MfaRepository
        + AuditRepository
        + Repository<Note>
{
}

/// A family of port implementations, chosen once by the composition root.
///
/// Services are generic over `A: Adapters` instead of over each port separately, and every call
/// is statically dispatched. Production uses Postgres, Argon2id, the OS random number generator
/// and the system clock; unit tests plug in fakes.
pub trait Adapters: Send + Sync + 'static {
    type Db: Database<Connection: Store, Transaction: Store>;
    type Hasher: PasswordHasher;
    type Tokens: TokenGenerator;
    type Crypto: Crypto;
    type Clock: Clock;
}

/// The configuration the use cases read, resolved once at startup by the composition root.
#[derive(Debug, Clone)]
pub struct Settings {
    pub app_name: String,
    pub sessions: SessionPolicy,
    pub tokens: TokenPolicy,
    pub require_email_verification: bool,
    /// Accounts whose address is still unverified this long after registering are deleted
    /// (`UNVERIFIED_ACCOUNT_TTL`); `None` keeps them.
    pub unverified_account_ttl: Option<time::Duration>,
    /// How long audit events are kept (`AUDIT_LOG_RETENTION`); `None` keeps them as long as the
    /// account exists.
    pub audit_retention: Option<time::Duration>,
    pub links: Links,
    /// Country calling codes phone numbers must start with to be texted. Empty allows every
    /// country.
    pub text_countries: Vec<domain::user::CallingCode>,
    /// The language of mail and texts. Users have no language of their own yet, so every mail is
    /// written in this one; API errors follow the request's `Accept-Language` instead.
    pub locale: Locale,
}

impl Settings {
    pub(crate) fn ensure_textable(
        &self,
        phone: &domain::user::PhoneNumber,
    ) -> Result<(), crate::AppError> {
        if self.text_countries.is_empty()
            || self.text_countries.iter().any(|code| code.covers(phone))
        {
            Ok(())
        } else {
            Err(crate::AppError::invalid(
                "phone",
                &ValidationError::new(
                    "phone_country_unsupported",
                    Message::new("validation-phone-country-unsupported"),
                ),
            ))
        }
    }
}

/// The port implementations and settings every service shares, built once by the composition
/// root and passed around as `Arc<Context<A>>`.
pub struct Context<A: Adapters> {
    pub db: A::Db,
    pub hasher: A::Hasher,
    pub tokens: A::Tokens,
    pub crypto: A::Crypto,
    pub clock: A::Clock,
    /// The ports picked from configuration at runtime are dynamically dispatched; see
    /// [`Mailer`].
    pub mailer: Arc<dyn Mailer>,
    pub texts: Arc<dyn TextSender>,
    pub identity_providers: Arc<dyn IdentityProviders>,
    /// Turns messages into text; mail and texts are rendered with it, and the API layer uses the
    /// same one for its responses.
    pub translator: Arc<dyn Translator>,
    pub settings: Settings,
}

impl<A: Adapters> Context<A> {
    pub(crate) fn voice(&self) -> Voice<'_> {
        Voice::new(&*self.translator, &self.settings.locale)
    }

    pub(crate) fn say(&self, message: &Message) -> String {
        self.voice().say(message)
    }

    /// An audit event about `user`, happening now, from `client`.
    pub(crate) fn event(
        &self,
        user: UserId,
        action: AuditAction,
        client: &ClientInfo,
    ) -> NewAuditEvent {
        NewAuditEvent::new(user, action, client.clone(), self.clock.now())
    }

    /// An audit event the actor caused on their own account.
    pub(crate) fn actor_event(&self, actor: &Actor, action: AuditAction) -> NewAuditEvent {
        self.event(actor.user_id, action, &actor.client)
            .by(actor.user_id)
    }
}
