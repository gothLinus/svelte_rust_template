use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use api::{
    cookie::AppCookie,
    rate_limit::{BucketStore, DEFAULT_MEMORY_MAX_KEYS, MemoryBuckets, RateLimits, Rates},
    router,
    state::AppState,
};
use application::{Adapters, Context, Services, Settings, mail::Links};
use axum::{
    Router,
    body::{Body, Bytes},
    extract::ConnectInfo,
    http::{
        HeaderMap, HeaderName, HeaderValue, Method, Request, StatusCode,
        header::{CONTENT_TYPE, COOKIE, SET_COOKIE},
    },
};
use domain::{
    i18n::{Locale, Message as Text},
    mfa::{hotp_code, totp_step},
    security::Crypto,
    session::SessionPolicy,
    user_token::TokenPolicy,
};
use http_body_util::BodyExt;
use i18n::Catalog;
use infrastructure::{
    config::{HttpConfig, PublicOrigin, RateLimitStore},
    crypto::{Argon2Hasher, Argon2Params, RandomTokens, RingCrypto},
    db::PostgresDatabase,
    testing::{FakeIdentityProviders, ManualClock, RecordingMailer, RecordingTexts},
};
use proto::{Message, v1};
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt;

pub const ORIGIN: &str = "http://localhost:5173";
pub const PROTOBUF: &str = "application/x-protobuf";
pub const PASSWORD: &str = "correct horse battery";

pub struct TestAdapters;

impl Adapters for TestAdapters {
    type Db = PostgresDatabase;
    type Hasher = Argon2Hasher;
    type Tokens = RandomTokens;
    type Crypto = RingCrypto;
    type Clock = ManualClock;
}

pub struct Options {
    pub require_email_verification: bool,
    pub rates: Option<Rates>,
    pub rate_limit_store: RateLimitStore,
    pub static_dir: Option<PathBuf>,
    pub secure_cookies: bool,
    pub cors_origins: Vec<&'static str>,
    pub hsts_max_age: Option<Duration>,
    pub trust_proxy: bool,
    pub max_body_bytes: usize,
    /// The catalog the app words texts with; the embedded one when `None`. Tests of other
    /// languages build one with `Catalog::from_sources`.
    pub catalog: Option<Arc<Catalog>>,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            require_email_verification: false,
            rates: None,
            rate_limit_store: RateLimitStore::Memory,
            static_dir: None,
            secure_cookies: false,
            cors_origins: Vec::new(),
            hsts_max_age: None,
            trust_proxy: false,
            max_body_bytes: 64 * 1024,
            catalog: None,
        }
    }
}

pub fn http_config(options: &Options) -> HttpConfig {
    HttpConfig {
        bind_addr: SocketAddr::from((Ipv4Addr::LOCALHOST, 0)),
        public_url: PublicOrigin::parse(ORIGIN).unwrap(),
        static_dir: options.static_dir.clone(),
        secure_cookies: options.secure_cookies,
        trusted_proxy_hops: usize::from(options.trust_proxy),
        hsts_max_age: options.hsts_max_age,
        cors_origins: options
            .cors_origins
            .iter()
            .map(|origin| PublicOrigin::parse(origin).unwrap())
            .collect(),
        request_timeout: Duration::from_secs(10),
        max_body_bytes: options.max_body_bytes,
        rate_limits: options.rates.is_some(),
        rates: options.rates.unwrap_or_default(),
        rate_limit_store: options.rate_limit_store,
        rate_limit_memory_max_keys: DEFAULT_MEMORY_MAX_KEYS,
        shutdown_timeout: Duration::from_secs(5),
    }
}

pub struct TestApp {
    router: Router,
    pub pool: PgPool,
    pub clock: ManualClock,
    pub mail: RecordingMailer,
    pub texts: RecordingTexts,
    pub providers: FakeIdentityProviders,
    /// The catalog the app words its texts with: tests take the expected text from it
    /// ([`TestApp::text`]) instead of repeating English.
    pub catalog: Arc<Catalog>,
    cookie: AppCookie,
}

impl TestApp {
    pub fn new(pool: PgPool) -> Self {
        Self::with(pool, Options::default())
    }

    pub fn with(pool: PgPool, options: Options) -> Self {
        let http = http_config(&options);
        let sessions = SessionPolicy::default();
        let clock = ManualClock::starting_now();
        let mail = RecordingMailer::new();
        let texts = RecordingTexts::new();
        let providers = FakeIdentityProviders::new();
        let catalog = options
            .catalog
            .unwrap_or_else(|| Arc::new(Catalog::embedded().unwrap()));
        let hasher = Argon2Hasher::new(Argon2Params {
            memory_kib: 64,
            iterations: 1,
            parallelism: 1,
            max_concurrent: 8,
            max_queued: 64,
        })
        .unwrap();

        let services = Arc::new(Services::new(Context::<TestAdapters> {
            db: PostgresDatabase::new(pool.clone()),
            hasher,
            tokens: RandomTokens,
            crypto: RingCrypto::new(&[7; 32]),
            clock: clock.clone(),
            mailer: Arc::new(mail.clone()),
            texts: Arc::new(texts.clone()),
            identity_providers: Arc::new(providers.clone()),
            translator: catalog.clone(),
            settings: Settings {
                app_name: "Acme".to_owned(),
                sessions,
                tokens: TokenPolicy::default(),
                require_email_verification: options.require_email_verification,
                unverified_account_ttl: None,
                links: Links::new(ORIGIN),
                text_countries: Vec::new(),
                locale: Locale::EN,
            },
        }));
        let limits = match options.rates {
            Some(rates) => RateLimits::new(
                rates,
                match options.rate_limit_store {
                    RateLimitStore::Memory => BucketStore::Memory(MemoryBuckets::new(
                        Arc::new(clock.clone()),
                        DEFAULT_MEMORY_MAX_KEYS,
                    )),
                    RateLimitStore::Postgres => BucketStore::Postgres(pool.clone()),
                },
                Arc::new(RingCrypto::new(&[7; 32])),
            ),
            None => RateLimits::disabled(),
        };
        let cookie = AppCookie::session(http.secure_cookies, sessions.absolute_lifetime);
        let state = AppState::new(services, cookie, Arc::new(limits), http.trusted_proxy_hops);

        Self {
            router: router::build(state, &http),
            pool,
            clock,
            mail,
            texts,
            providers,
            catalog,
            cookie,
        }
    }

    pub fn text(&self, id: &'static str) -> String {
        self.catalog.text_of(id)
    }

    pub fn say(&self, message: &Text) -> String {
        self.catalog.text(message)
    }

    pub async fn send(&self, request: TestRequest) -> TestResponse {
        let request = request.build(self.cookie.name());
        let response = self.router.clone().oneshot(request).await.unwrap();

        let status = response.status();
        let headers = response.headers().clone();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let text = String::from_utf8_lossy(&bytes).into_owned();
        let is_json = headers
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.starts_with("application/") && value.contains("json"));
        let body = if is_json {
            serde_json::from_slice(&bytes).unwrap()
        } else {
            Value::Null
        };

        TestResponse {
            status,
            headers,
            body,
            bytes,
            text,
            cookie_name: self.cookie.name(),
        }
    }

    pub async fn register(&self, email: &str) -> String {
        let response = self
            .send(
                TestRequest::post("/api/v1/auth/register").proto(&v1::RegisterRequest {
                    email: email.to_owned(),
                    username: email.split('@').next().unwrap().to_owned(),
                    password: PASSWORD.to_owned(),
                }),
            )
            .await;
        assert_eq!(response.status, StatusCode::CREATED, "{}", response.text);
        response.session_token().expect("no session cookie")
    }

    pub async fn login(&self, email: &str, password: &str) -> TestResponse {
        self.send(
            TestRequest::post("/api/v1/auth/login").proto(&v1::LoginRequest {
                identifier: email.to_owned(),
                password: password.to_owned(),
            }),
        )
        .await
    }

    pub async fn admin(&self, email: &str) -> String {
        let token = self.register(email).await;
        sqlx::query(
            "insert into user_roles (user_id, role) \
             select id, 'admin' from users where email = $1",
        )
        .bind(email)
        .execute(&self.pool)
        .await
        .unwrap();
        token
    }

    pub async fn me(&self, token: &str) -> v1::Me {
        let response = self
            .send(TestRequest::get("/api/v1/me").session(token))
            .await;
        assert_eq!(response.status, StatusCode::OK, "{}", response.text);
        response.decode()
    }

    pub async fn user_id(&self, token: &str) -> String {
        user(&self.me(token).await).id.clone()
    }

    /// Registers `email` like [`TestApp::register`] and marks the address verified, which adding a
    /// sign-in method requires.
    pub async fn register_verified(&self, email: &str) -> String {
        let token = self.register(email).await;
        self.verify_email(email).await;
        token
    }

    pub async fn verify_email(&self, email: &str) {
        sqlx::query("update users set email_verified_at = now() where email = $1")
            .bind(email)
            .execute(&self.pool)
            .await
            .unwrap();
    }

    pub async fn set_phone(&self, email: &str, phone: &str) {
        sqlx::query("update users set phone = $2, phone_verified_at = now() where email = $1")
            .bind(email)
            .bind(phone)
            .execute(&self.pool)
            .await
            .unwrap();
    }

    pub async fn enable_totp(&self, token: &str) -> Vec<u8> {
        let setup = self
            .send(TestRequest::post("/api/v1/me/mfa/totp").session(token))
            .await;
        assert_eq!(setup.status, StatusCode::OK, "{}", setup.text);
        let secret = base32_decode(&setup.decode::<v1::TotpSetup>().secret);
        let confirmed = self
            .send(
                TestRequest::post("/api/v1/me/mfa/totp/confirm")
                    .session(token)
                    .proto(&v1::CodeRequest {
                        code: self.totp(&secret),
                    }),
            )
            .await;
        assert_eq!(confirmed.status, StatusCode::OK, "{}", confirmed.text);
        secret
    }

    pub fn totp(&self, secret: &[u8]) -> String {
        let step = totp_step(domain::clock::Clock::now(&self.clock));
        hotp_code(&RingCrypto::new(&[0; 32]).hmac_sha1(secret, &step.to_be_bytes()))
    }

    pub fn mailed_code(&self, email: &str) -> String {
        let mail = self.mail.last_to(email).expect("no mail was sent");
        code_in(&mail.body)
    }

    pub fn mailed_token(&self, email: &str) -> String {
        let mail = self.mail.last_to(email).expect("no mail was sent");
        let (_, rest) = mail
            .body
            .split_once("#token=")
            .expect("no link in the mail");
        rest.split_whitespace().next().unwrap().to_owned()
    }
}

pub fn user(me: &v1::Me) -> &v1::User {
    me.user.as_ref().expect("a `Me` without a user")
}

/// Decodes the unpadded RFC 4648 base32 the setup response shows the secret in.
pub fn base32_decode(input: &str) -> Vec<u8> {
    const ALPHABET: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let mut bits = 0u32;
    let mut count = 0;
    let mut out = Vec::new();
    for c in input.chars() {
        bits = (bits << 5) | u32::try_from(ALPHABET.find(c).unwrap()).unwrap();
        count += 5;
        if count >= 8 {
            count -= 8;
            out.push(u8::try_from((bits >> count) & 0xff).unwrap());
        }
    }
    out
}

pub struct TestRequest {
    method: Method,
    uri: String,
    headers: Vec<(HeaderName, HeaderValue)>,
    body: Body,
    session: Option<String>,
    cookies: Vec<(String, String)>,
    peer: Option<IpAddr>,
    csrf_headers: bool,
}

impl TestRequest {
    pub fn new(method: Method, uri: &str) -> Self {
        Self {
            method,
            uri: uri.to_owned(),
            headers: Vec::new(),
            body: Body::empty(),
            session: None,
            cookies: Vec::new(),
            peer: Some(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1))),
            csrf_headers: true,
        }
    }

    pub fn get(uri: &str) -> Self {
        Self::new(Method::GET, uri)
    }

    pub fn post(uri: &str) -> Self {
        Self::new(Method::POST, uri)
    }

    pub fn put(uri: &str) -> Self {
        Self::new(Method::PUT, uri)
    }

    pub fn patch(uri: &str) -> Self {
        Self::new(Method::PATCH, uri)
    }

    pub fn delete(uri: &str) -> Self {
        Self::new(Method::DELETE, uri)
    }

    pub fn proto(self, message: &impl Message) -> Self {
        self.body(PROTOBUF, message.encode_to_vec())
    }

    pub fn json(self, body: &Value) -> Self {
        self.body("application/json", body.to_string())
    }

    pub fn body(mut self, content_type: &str, body: impl Into<Body>) -> Self {
        self = self.header(CONTENT_TYPE.as_str(), content_type);
        self.body = body.into();
        self
    }

    pub fn header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((
            HeaderName::try_from(name).unwrap(),
            HeaderValue::try_from(value).unwrap(),
        ));
        self
    }

    pub fn session(mut self, token: &str) -> Self {
        self.session = Some(token.to_owned());
        self
    }

    pub fn cookie(mut self, name: &str, value: &str) -> Self {
        self.cookies.push((name.to_owned(), value.to_owned()));
        self
    }

    pub fn with_peer(mut self, ip: &str) -> Self {
        self.peer = Some(ip.parse().unwrap());
        self
    }

    pub fn without_csrf_headers(mut self) -> Self {
        self.csrf_headers = false;
        self
    }

    fn build(self, cookie_name: &str) -> Request<Body> {
        let mut builder = Request::builder().method(&self.method).uri(&self.uri);
        if self.csrf_headers && !self.method.is_safe() {
            builder = builder
                .header("x-requested-with", "fetch")
                .header("origin", ORIGIN);
        }
        let mut cookies: Vec<String> = self
            .cookies
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect();
        if let Some(token) = &self.session {
            cookies.push(format!("{cookie_name}={token}"));
        }
        if !cookies.is_empty() {
            builder = builder.header(COOKIE, cookies.join("; "));
        }
        for (name, value) in self.headers {
            builder = builder.header(name, value);
        }
        let mut request = builder.body(self.body).unwrap();
        if let Some(ip) = self.peer {
            request
                .extensions_mut()
                .insert(ConnectInfo(SocketAddr::new(ip, 40_000)));
        }
        request
    }
}

pub struct TestResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Value,
    pub bytes: Bytes,
    pub text: String,
    cookie_name: &'static str,
}

impl TestResponse {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).map(|value| value.to_str().unwrap())
    }

    #[track_caller]
    pub fn decode<M: Message + Default>(&self) -> M {
        assert_eq!(
            self.header("content-type"),
            Some(PROTOBUF),
            "expected a protobuf response, got status {}: {}",
            self.status,
            self.text
        );
        M::decode(&self.bytes[..]).unwrap_or_else(|err| {
            panic!(
                "status {}: the body is not a valid {}: {err}",
                self.status,
                std::any::type_name::<M>()
            )
        })
    }

    pub fn code(&self) -> &str {
        self.body["code"]
            .as_str()
            .unwrap_or_else(|| panic!("not a problem document: {}", self.text))
    }

    pub fn session_cookie(&self) -> Option<&str> {
        let prefix = format!("{}=", self.cookie_name);
        self.headers
            .get_all(SET_COOKIE)
            .iter()
            .map(|value| value.to_str().unwrap())
            .find(|cookie| cookie.starts_with(&prefix))
    }

    pub fn session_token(&self) -> Option<String> {
        let (_, rest) = self.session_cookie()?.split_once('=')?;
        let token = rest.split(';').next()?;
        (!token.is_empty()).then(|| token.to_owned())
    }

    pub fn cookie(&self, name: &str) -> Option<String> {
        let prefix = format!("{name}=");
        self.headers
            .get_all(SET_COOKIE)
            .iter()
            .map(|value| value.to_str().unwrap())
            .find(|cookie| cookie.starts_with(&prefix))
            .map(|cookie| cookie[prefix.len()..].split(';').next().unwrap().to_owned())
    }

    pub fn clears_session(&self) -> bool {
        self.session_cookie()
            .is_some_and(|cookie| self.session_token().is_none() && cookie.contains("Max-Age=0"))
    }

    #[track_caller]
    pub fn assert_problem(&self, status: StatusCode, code: &str) {
        assert_eq!(self.status, status, "{}", self.text);
        assert_eq!(
            self.header("content-type"),
            Some("application/problem+json"),
            "{}",
            self.text
        );
        assert_eq!(self.code(), code, "{}", self.text);
        assert_eq!(self.body["status"], status.as_u16());
    }

    #[track_caller]
    pub fn assert_field_error(&self, field: &str, code: &str) {
        self.assert_problem(StatusCode::UNPROCESSABLE_ENTITY, "validation_failed");
        assert_eq!(self.body["errors"][0]["field"], field, "{}", self.text);
        assert_eq!(self.body["errors"][0]["code"], code, "{}", self.text);
    }

    /// A wrong, expired or used one-time code: the same answer for every kind of code.
    #[track_caller]
    pub fn assert_invalid_code(&self) {
        self.assert_field_error("code", "invalid_code");
    }
}

pub fn default_permissions() -> Vec<proto::v1::Permission> {
    domain::rbac::DEFAULT_USER_PERMISSIONS
        .iter()
        .map(|&permission| api::wire::permission_message(permission.into()))
        .collect()
}

/// The six-digit one-time code (shown as `123 456`) in a mail body: the line that holds nothing
/// else, so it does not depend on the wording around it.
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

const GERMAN: &str = "\
error-not-found = nicht gefunden
error-validation-failed = die Anfrage ist ungültig
error-internal = ein unerwarteter Fehler ist aufgetreten
error-forbidden = keine Berechtigung
http-status-404 = Nicht gefunden
http-status-422 = Nicht verarbeitbar
http-status-503 = Dienst nicht verfügbar
http-method-not-allowed = Methode nicht erlaubt
http-rate-limited = zu viele Versuche, bitte später erneut versuchen
http-timeout = die Anfrage hat zu lange gedauert
http-csrf-header-required = zustandsändernde Anfragen brauchen den Header X-Requested-With
validation-required = dieses Feld ist erforderlich
validation-password-too-short = muss mindestens { $min } Zeichen lang sein
";

pub fn catalog_with_german() -> Arc<Catalog> {
    Arc::new(Catalog::embedded_with(&[("de", vec![("errors.ftl", GERMAN)])]).unwrap())
}
