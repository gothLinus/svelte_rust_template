#![expect(
    clippy::unused_async_trait_impl,
    reason = "the fakes implement async ports with synchronous code"
)]

pub mod fakes;
pub mod memory;
mod memory_sign_in;

use std::sync::Arc;

use application::{
    Adapters, Context, Services, Settings,
    auth::{Authenticated, LoginOutcome, Registered, SignedIn, dto::RegisterRequest},
    dto::SecretInput,
    mail::Links,
};
use domain::{
    clock::Clock,
    database::Database,
    i18n::{Locale, Message, Translator},
    rbac::RoleName,
    secret::Secret,
    session::{ClientInfo, SessionPolicy},
    user::{UserId, UserRepository},
    user_token::TokenPolicy,
};
use fakes::{FakeClock, FakeCrypto, FakeHasher, FakeMailer, FakeProviders, FakeTexts, FakeTokens};
use i18n::Catalog;
use memory::MemoryDb;

pub const PASSWORD: &str = "correct horse battery";
pub const APP_URL: &str = "https://app.test";

pub struct Fakes;

impl Adapters for Fakes {
    type Db = MemoryDb;
    type Hasher = FakeHasher;
    type Tokens = FakeTokens;
    type Crypto = FakeCrypto;
    type Clock = FakeClock;
}

pub struct Fixture {
    pub services: Services<Fakes>,
    pub db: MemoryDb,
    pub clock: FakeClock,
    pub hasher: FakeHasher,
    pub mail: FakeMailer,
    pub texts: FakeTexts,
    pub providers: FakeProviders,
    pub catalog: Arc<Catalog>,
}

pub fn settings() -> Settings {
    Settings {
        app_name: "Acme".to_owned(),
        sessions: SessionPolicy::default(),
        tokens: TokenPolicy::default(),
        require_email_verification: false,
        unverified_account_ttl: None,
        links: Links::new(APP_URL),
        text_countries: Vec::new(),
        locale: Locale::EN,
    }
}

impl Fixture {
    pub fn new() -> Self {
        Self::with(|_| {})
    }

    pub fn with(configure: impl FnOnce(&mut Settings)) -> Self {
        Self::with_catalog(Arc::new(Catalog::embedded().unwrap()), configure)
    }

    /// Words mail and texts with `catalog`, such as one with a second language that `configure`
    /// sets as `Settings::locale`.
    pub fn with_catalog(catalog: Arc<Catalog>, configure: impl FnOnce(&mut Settings)) -> Self {
        let mut settings = settings();
        configure(&mut settings);

        let db = MemoryDb::default();
        let clock = FakeClock::default();
        let hasher = FakeHasher::default();
        let mail = FakeMailer::default();
        let texts = FakeTexts::default();
        let providers = FakeProviders::default();
        let services = Services::new(Context::<Fakes> {
            db: db.clone(),
            hasher: hasher.clone(),
            tokens: FakeTokens::default(),
            crypto: FakeCrypto::default(),
            clock: clock.clone(),
            mailer: Arc::new(mail.clone()),
            texts: Arc::new(texts.clone()),
            identity_providers: Arc::new(providers.clone()),
            translator: Arc::<Catalog>::clone(&catalog),
            settings,
        });

        Self {
            services,
            db,
            clock,
            hasher,
            mail,
            texts,
            providers,
            catalog,
        }
    }

    pub fn text(&self, id: &'static str) -> String {
        self.catalog.text_of(id)
    }

    pub fn say(&self, message: &Message) -> String {
        self.catalog.translate(&Locale::EN, message)
    }

    pub async fn register(&self, email: &str) -> SignedIn {
        match self
            .services
            .auth
            .register(register_request(email), ClientInfo::default())
            .await
            .unwrap()
        {
            Registered::SignedIn(signed_in) => *signed_in,
            Registered::VerificationPending { .. } => panic!("verification is required"),
        }
    }

    pub async fn authenticate(&self, token: &Secret) -> Authenticated {
        self.services
            .auth
            .authenticate(token)
            .await
            .unwrap()
            .expect("the token is not valid")
    }

    pub async fn user(&self, email: &str) -> Authenticated {
        let signed_in = self.register(email).await;
        self.verify(UserId::from_uuid(signed_in.me.user.id)).await;
        self.authenticate(&signed_in.token).await
    }

    pub async fn unverified_user(&self, email: &str) -> Authenticated {
        let signed_in = self.register(email).await;
        self.authenticate(&signed_in.token).await
    }

    pub async fn verify(&self, user: UserId) {
        self.db
            .connection()
            .await
            .unwrap()
            .mark_user_email_verified(user, self.clock.now())
            .await
            .unwrap();
    }

    pub async fn admin(&self, email: &str) -> Authenticated {
        let signed_in = self.register(email).await;
        let id = UserId::from_uuid(signed_in.me.user.id);
        self.verify(id).await;
        self.db.with(|state| {
            state.user_roles.insert((id, RoleName::ADMIN));
        });
        self.authenticate(&signed_in.token).await
    }
}

impl Fixture {
    pub fn totp_now(&self, user: UserId) -> String {
        use domain::{
            mfa::{hotp_code, totp_step},
            security::Crypto,
        };
        let sealed = self
            .db
            .with(|state| state.totp[&user].sealed_secret.clone());
        let secret = FakeCrypto::default()
            .open(&sealed, format!("totp:{user}").as_bytes())
            .unwrap();
        let step = totp_step(self.clock.now());
        hotp_code(&FakeCrypto::default().hmac_sha1(&secret, &step.to_be_bytes()))
    }

    pub async fn enable_totp(&self, actor: &application::actor::Actor) -> Vec<String> {
        let setup = self.services.mfa.start_totp(actor).await.unwrap();
        let account = self.db.with(|state| {
            state.users[&actor.user_id]
                .email()
                .as_str()
                .replace('@', "%40")
        });
        assert!(
            setup
                .uri
                .starts_with(&format!("otpauth://totp/Acme:{account}?secret=")),
            "{}",
            setup.uri
        );
        let current = self.totp_now(actor.user_id);
        self.services
            .mfa
            .confirm_totp(
                actor,
                application::mfa::dto::CodeRequest {
                    code: secret(&current),
                },
            )
            .await
            .unwrap()
            .recovery_codes
            .expect("recovery codes for the first second step")
    }
}

pub fn register_request(email: &str) -> RegisterRequest {
    RegisterRequest {
        email: email.to_owned(),
        username: email.split('@').next().unwrap_or(email).to_owned(),
        password: secret(PASSWORD),
    }
}

#[track_caller]
pub fn code_in(body: &str) -> String {
    let code: String = body
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && line.chars().all(|c| c.is_ascii_digit() || c == ' '))
        .unwrap_or_else(|| panic!("no code in the mail: {body}"))
        .chars()
        .filter(char::is_ascii_digit)
        .collect();
    assert_eq!(code.len(), 6, "{body}");
    code
}

pub fn secret(value: &str) -> SecretInput {
    SecretInput(Secret::new(value))
}

pub trait Outcome {
    fn signed_in(self) -> SignedIn;
    fn mfa_required(self) -> application::auth::MfaRequired;
}

impl Outcome for LoginOutcome {
    fn signed_in(self) -> SignedIn {
        match self {
            Self::SignedIn(signed_in) => *signed_in,
            Self::MfaRequired(_) => panic!("a second step is required"),
        }
    }

    fn mfa_required(self) -> application::auth::MfaRequired {
        match self {
            Self::MfaRequired(required) => required,
            Self::SignedIn(_) => panic!("signed in without a second step"),
        }
    }
}

#[track_caller]
pub fn assert_invalid_code(err: &application::AppError) {
    let application::AppError::Validation(errors) = err else {
        panic!("expected a field error, got {err:?}");
    };
    assert_eq!(errors.fields()[0].field, "code");
    assert_eq!(errors.fields()[0].code, "invalid_code");
}
