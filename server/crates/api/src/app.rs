//! The composition root: the only place that picks concrete implementations and wires them
//! together.
//!
//! [`Production`] fixes the [`Adapters`]: Postgres, Argon2id, the OS random number generator,
//! ring and the system clock. [`serve`] adds what configuration chooses: the
//! mail transport (SMTP or log) and text channels behind the outbox, the social sign-in providers,
//! and the rate limit buckets (in memory or Postgres). It
//! then builds the router, starts the maintenance job and runs until a signal arrives. The other
//! functions back the `api` subcommands (see `main.rs`).

use std::{
    future::IntoFuture,
    io,
    net::SocketAddr,
    sync::Arc,
    time::{Duration, Instant},
};

use application::{Adapters, Context, Services, Settings, mail::Links};
use axum::{Router, serve::ListenerExt};
use domain::{
    database::{Database, Transaction},
    error::StorageError,
    object_store::ObjectStoreError,
    rbac::{RbacRepository, RoleName},
    security::HashError,
    session::SessionRepository,
    user::{Email, UserRepository},
};
use i18n::{Catalog, CatalogError, DEFAULT_LOCALE};
use infrastructure::{
    clock::SystemClock,
    config::{Config, ConfigError, DotenvError, MailTransport, RateLimitStore},
    crypto::{Argon2Hasher, RandomTokens, RingCrypto},
    db::{self, ClusterLock, PgPool, PoolError, PostgresDatabase},
    mail::{self, InvalidSmtpUrl},
    oauth::{self, OAuthProviders},
    object_store::{BucketStatus, S3ObjectStore},
    outbox::Outbox,
    text,
};
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::oneshot,
    task::JoinError,
    time::{timeout, timeout_at},
};

use crate::{
    background::Background,
    cookie::AppCookie,
    jobs,
    rate_limit::{BucketStore, MemoryBuckets, RateLimits},
    router, shutdown,
    state::AppState,
    telemetry::TelemetryError,
};

const DB_CLOSE_TIMEOUT: Duration = Duration::from_secs(2);
const HEALTHCHECK_TIMEOUT: Duration = Duration::from_secs(5);

/// The production adapters: Postgres, Argon2id, the OS random number generator, ring and the system
/// clock.
pub struct Production;

impl Adapters for Production {
    type Db = PostgresDatabase;
    type Hasher = Argon2Hasher;
    type Tokens = RandomTokens;
    type Crypto = RingCrypto;
    type Clock = SystemClock;
    type Objects = S3ObjectStore;
}

/// Everything that can stop a command from starting or finishing. `main.rs` prints it with its
/// source chain and exits non-zero.
#[derive(Debug, Error)]
pub enum StartupError {
    #[error(transparent)]
    Dotenv(#[from] DotenvError),
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error("failed to initialise logging")]
    Telemetry(#[source] TelemetryError),
    #[error("failed to start the async runtime")]
    Runtime(#[source] io::Error),
    #[error(transparent)]
    Database(#[from] PoolError),
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error(transparent)]
    Mail(#[from] InvalidSmtpUrl),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error("failed to set up password hashing")]
    PasswordHasher(#[from] HashError),
    #[error("failed to set up the HTTP client for sign-in providers and texting")]
    HttpClient(#[source] oauth::HttpClientError),
    #[error("the object store at {endpoint} (bucket `{bucket}`) is not usable")]
    ObjectStore {
        endpoint: String,
        bucket: String,
        #[source]
        source: ObjectStoreError,
    },
    #[error("failed to bind to {addr}")]
    Bind {
        addr: SocketAddr,
        #[source]
        source: io::Error,
    },
    #[error("the HTTP server failed")]
    Serve(#[source] io::Error),
    #[error("`{0}` is not a valid email address")]
    InvalidEmail(String),
    #[error("no account uses {0}; register it first")]
    UnknownUser(String),
    #[error("the health check failed: {0}")]
    Unhealthy(String),
}

pub fn settings(config: &Config) -> Settings {
    Settings {
        app_name: config.auth.app_name.clone(),
        sessions: config.auth.sessions,
        tokens: config.auth.tokens,
        require_email_verification: config.auth.require_email_verification,
        unverified_account_ttl: config.auth.unverified_account_ttl,
        audit_retention: config.auth.audit_retention,
        links: Links::new(config.http.public_url.as_str()),
        text_countries: config.texts.allowed_countries.clone(),
        locale: DEFAULT_LOCALE,
        file_quota: config.storage.quota_per_user,
    }
}

/// The object store `STORAGE_*` names, with its bucket checked (and created if missing), so wrong
/// credentials or an unreachable store stop the server at startup like an unreachable database
/// does, rather than failing the first upload.
///
/// # Errors
///
/// Fails if the store cannot be reached, refuses the credentials, or will not create the bucket.
pub async fn object_store(config: &Config) -> Result<S3ObjectStore, StartupError> {
    let failed = |source| StartupError::ObjectStore {
        endpoint: config.storage.endpoint.to_string(),
        bucket: config.storage.bucket.clone(),
        source,
    };
    let store = S3ObjectStore::new(&config.storage).map_err(failed)?;
    match store.ensure_bucket().await.map_err(failed)? {
        BucketStatus::Existed => {}
        BucketStatus::Created => {
            tracing::info!(bucket = %config.storage.bucket, "created the object store's bucket");
        }
    }
    Ok(store)
}

/// The [`RingCrypto`] keyed with `SECRET_KEY`. During a key rotation it also opens what
/// `SECRET_KEY_PREVIOUS` sealed.
pub fn crypto(config: &Config) -> RingCrypto {
    let crypto = RingCrypto::new(config.auth.secret_key.bytes());
    match &config.auth.previous_secret_key {
        Some(previous) => crypto.with_previous_key(previous.bytes()),
        None => crypto,
    }
}

pub fn session_cookie(config: &Config) -> AppCookie {
    AppCookie::session(
        config.http.secure_cookies,
        config.auth.sessions.absolute_lifetime,
    )
}

/// The limits `RATE_LIMITS_ENABLED` and `RATE_LIMIT_*` ask for, with their buckets where
/// `RATE_LIMIT_STORE` says and their keys digested with `SECRET_KEY`.
pub fn rate_limits(config: &Config, pool: &PgPool) -> RateLimits {
    let keys = Arc::new(crypto(config));
    let config = &config.http;
    if !config.rate_limits {
        return RateLimits::disabled();
    }
    let store = match config.rate_limit_store {
        RateLimitStore::Memory => BucketStore::Memory(MemoryBuckets::new(
            Arc::new(SystemClock),
            config.rate_limit_memory_max_keys,
        )),
        RateLimitStore::Postgres => BucketStore::Postgres(pool.clone()),
    };
    RateLimits::new(config.rates, store, keys)
}

/// Runs the server until SIGINT or SIGTERM, then shuts down gracefully within `SHUTDOWN_TIMEOUT`:
/// in-flight requests finish, queued mail and texts are handed over, the pool is closed. Whatever
/// is still running at the deadline is cut off, so the process exits before the orchestrator's
/// grace period ends and kills it mid-step.
///
/// # Errors
///
/// Fails before serving if the translation catalog is invalid, the database is unreachable or its
/// migrations fail (when `DATABASE_RUN_MIGRATIONS` asks for them), the mail or HTTP client setup is
/// invalid, the Argon2 parameters are unusable, or the address cannot be
/// bound. Afterwards it fails only if the HTTP server itself does.
pub async fn serve(config: Config) -> Result<(), StartupError> {
    // The catalog is compiled in, so a broken one is a bug of this build: refuse to start rather
    // than answer with message ids.
    let translator = Arc::new(Catalog::embedded()?);
    let pool = db::connect(&config.database).await?;
    if config.database.run_migrations {
        db::migrate(&pool).await?;
        tracing::info!("database migrations are up to date");
    }
    let objects = object_store(&config).await?;

    let mail_transport = mail::transport(&config.mail)?;
    if matches!(config.mail.transport, MailTransport::Log) {
        tracing::warn!("MAIL_TRANSPORT=log: mail is written to the log, not sent");
    }

    let http = oauth::http_client().map_err(StartupError::HttpClient)?;
    let text_transport = text::start(&config.texts, &http);
    let outbox = Outbox::new(pool.clone(), Arc::new(crypto(&config)));
    let mailer = Arc::new(outbox.mailer());
    let texts = Arc::new(outbox.texts(text_transport.channels()));
    let outbox_worker = outbox.start_worker(mail_transport, text_transport);
    let identity_providers =
        OAuthProviders::new(&config.oauth, config.http.public_url.clone(), http);
    for provider in &config.oauth.providers {
        tracing::info!(
            provider = provider.provider.id(),
            redirect_uri = identity_providers.redirect_uri(provider.provider),
            "social sign-in enabled"
        );
    }

    let hasher = Argon2Hasher::new(config.auth.argon2)?;
    let services = Arc::new(Services::new(Context::<Production> {
        db: PostgresDatabase::new(pool.clone()),
        hasher,
        tokens: RandomTokens,
        crypto: crypto(&config),
        clock: SystemClock,
        objects,
        mailer,
        texts,
        identity_providers: Arc::new(identity_providers),
        translator,
        settings: settings(&config),
    }));
    let limits = Arc::new(rate_limits(&config, &pool));
    let maintenance = tokio::spawn(jobs::maintenance(
        Arc::clone(&services),
        Arc::clone(&limits),
        Some(ClusterLock::new(pool.clone(), ClusterLock::MAINTENANCE)),
    ));
    let background = Background::spawning();
    let state = AppState::new(
        services,
        session_cookie(&config),
        limits,
        config.http.trusted_proxy_hops,
    )
    .with_background(background.clone());
    let app = router::build(state, &config.http);

    let addr = config.http.bind_addr;
    let listener = TcpListener::bind(addr)
        .await
        .map_err(|source| StartupError::Bind { addr, source })?;
    tracing::info!(%addr, url = %config.http.public_url, "listening");

    let (served, deadline) = run_server(listener, app, config.http.shutdown_timeout).await;
    tracing::info!("server stopped, shutting down");

    maintenance.abort();
    // Work requests handed off (see `Background`), then what is due in the outbox. What the
    // deadline cuts off stays in the outbox for the next start.
    let _aborted = maintenance.await;
    let drain_deadline = deadline.checked_sub(DB_CLOSE_TIMEOUT).unwrap_or(deadline);
    if timeout_at(drain_deadline.into(), background.drain())
        .await
        .is_err()
    {
        tracing::warn!("shutdown deadline reached with background work still running");
    }
    if timeout_at(drain_deadline.into(), outbox_worker.finish())
        .await
        .is_err()
    {
        tracing::warn!("shutdown deadline reached with messages still in the outbox");
    }
    if timeout_at(
        deadline.max(Instant::now() + DB_CLOSE_TIMEOUT).into(),
        pool.close(),
    )
    .await
    .is_err()
    {
        tracing::warn!("timed out closing the database pool");
    }

    served.map_err(StartupError::Serve)
}

async fn run_server(
    listener: TcpListener,
    app: Router,
    shutdown_timeout: Duration,
) -> (io::Result<()>, Instant) {
    let (signalled, on_signal) = oneshot::channel();
    // Responses leave as soon as they are written. Compressed bodies are sent in several writes,
    // and Nagle's algorithm would hold the last one back until the client acks the previous (which
    // delayed acks can put off by 40 ms).
    let listener = listener.tap_io(|stream| {
        if let Err(err) = stream.set_nodelay(true) {
            tracing::trace!(%err, "failed to set TCP_NODELAY");
        }
    });
    let mut task = tokio::spawn(
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(async move {
            shutdown::signal().await;
            let _receiver_gone = signalled.send(Instant::now());
        })
        .into_future(),
    );
    let flatten = |joined: Result<io::Result<()>, JoinError>| {
        joined.unwrap_or_else(|err| Err(io::Error::other(err)))
    };

    let deadline = tokio::select! {
        joined = &mut task => return (flatten(joined), Instant::now()),
        at = on_signal => at.unwrap_or_else(|_| Instant::now()) + shutdown_timeout,
    };
    if let Ok(joined) = timeout_at(deadline.into(), &mut task).await {
        (flatten(joined), deadline)
    } else {
        tracing::warn!("shutdown deadline reached with requests still in flight");
        task.abort();
        (Ok(()), deadline)
    }
}

/// Applies pending migrations and returns, whatever `DATABASE_RUN_MIGRATIONS` says; that setting
/// only governs [`serve`].
///
/// # Errors
///
/// Fails if the database cannot be reached or a migration fails.
pub async fn migrate(config: &Config) -> Result<(), StartupError> {
    let pool = db::connect(&config.database).await?;
    db::migrate(&pool).await?;
    pool.close().await;
    Ok(())
}

/// Gives an existing account the `admin` role. The first admin has to be made this way:
/// `just create-admin alice@example.com`. The account's sessions are flagged for rotation, as
/// after any role change.
///
/// # Errors
///
/// Fails if `email` is not a valid address, no account uses it, or the database fails. Granting
/// the role to an admin again is not an error.
pub async fn create_admin(config: &Config, email: &str) -> Result<(), StartupError> {
    let email = Email::parse(email).map_err(|_| StartupError::InvalidEmail(email.to_owned()))?;
    let pool = db::connect(&config.database).await?;
    let database = PostgresDatabase::new(pool.clone());

    let mut tx = database.transaction().await?;
    let user = tx
        .find_user_by_email(&email)
        .await?
        .ok_or_else(|| StartupError::UnknownUser(email.to_string()))?;
    if tx.grant_role(user.id(), &RoleName::ADMIN).await? {
        tx.flag_user_sessions_for_rotation(user.id()).await?;
    }
    tx.commit().await?;
    pool.close().await;

    tracing::info!(user_id = %user.id(), "granted the admin role");
    Ok(())
}

/// Asks the running server for `/health/live`. For container health checks: the runtime image has
/// no curl.
///
/// # Errors
///
/// Fails unless the server answers `200` within five seconds. An unspecified `bind_addr`
/// (`0.0.0.0`) is probed on the loopback address.
pub async fn healthcheck(bind_addr: SocketAddr) -> Result<(), StartupError> {
    let addr = if bind_addr.ip().is_unspecified() {
        SocketAddr::from(([127, 0, 0, 1], bind_addr.port()))
    } else {
        bind_addr
    };

    let probe = async {
        let mut stream = TcpStream::connect(addr).await?;
        stream
            .write_all(b"GET /health/live HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .await?;
        let mut head = [0u8; 12];
        stream.read_exact(&mut head).await?;
        Ok::<_, io::Error>(head)
    };

    match timeout(HEALTHCHECK_TIMEOUT, probe).await {
        Ok(Ok(head)) if head.starts_with(b"HTTP/1.1 200") => Ok(()),
        Ok(Ok(head)) => Err(StartupError::Unhealthy(
            String::from_utf8_lossy(&head).into_owned(),
        )),
        Ok(Err(err)) => Err(StartupError::Unhealthy(err.to_string())),
        Err(_) => Err(StartupError::Unhealthy("timed out".to_owned())),
    }
}
