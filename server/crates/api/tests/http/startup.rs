use std::{collections::HashMap, sync::Arc, time::Duration};

use api::{
    app::{self, StartupError},
    jobs,
    rate_limit::RateLimits,
};
use application::{Context, Services, mail::Links};
use axum::http::StatusCode;
use infrastructure::{
    config::Config,
    crypto::{Argon2Hasher, Argon2Params, RandomTokens},
    db::PostgresDatabase,
    testing::{ManualClock, RecordingMailer},
};
use sqlx::PgPool;
use tokio::{io::AsyncWriteExt, net::TcpListener};

use crate::support::{TestAdapters, TestApp, TestRequest, user};

fn config(pool: &PgPool) -> Config {
    let base = std::env::var("DATABASE_URL").unwrap();
    let database = pool.connect_options().get_database().unwrap().to_owned();
    let (server, _) = base.rsplit_once('/').unwrap();
    let vars: HashMap<&str, String> = [
        ("APP_URL", "http://localhost:5173".to_owned()),
        ("DATABASE_URL", format!("{server}/{database}")),
        ("MAIL_FROM", "App <noreply@example.com>".to_owned()),
        ("RATE_LIMITS_ENABLED", "false".to_owned()),
        (
            "SECRET_KEY",
            "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=".to_owned(),
        ),
    ]
    .into();
    Config::from_lookup(|name| vars.get(name).cloned()).unwrap()
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn create_admin_promotes_an_existing_account(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    let token = app.register("alice@example.com").await;
    let config = config(&pool);

    app::migrate(&config).await.unwrap();
    app::create_admin(&config, "Alice@Example.com")
        .await
        .unwrap();

    let me = app
        .send(TestRequest::get("/api/v1/me").session(&token))
        .await;
    assert_eq!(me.status, StatusCode::OK);
    assert!(me.session_token().is_some());
    assert_eq!(user(&me.decode()).roles, ["admin", "user"]);

    app::create_admin(&config, "alice@example.com")
        .await
        .unwrap();

    assert!(matches!(
        app::create_admin(&config, "bob@example.com").await,
        Err(StartupError::UnknownUser(_))
    ));
    assert!(matches!(
        app::create_admin(&config, "not an email").await,
        Err(StartupError::InvalidEmail(_))
    ));
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn settings_follow_the_config(pool: PgPool) {
    let config = config(&pool);

    let settings = app::settings(&config);
    assert!(!settings.require_email_verification);
    assert_eq!(settings.sessions, config.auth.sessions);
    // `COOKIE_SECURE` defaults to true; localhost counts as a secure context.
    assert_eq!(app::session_cookie(&config).name(), "__Host-session");

    let limits = app::rate_limits(&config, &pool);
    for _ in 0..100 {
        limits
            .check_ip(api::rate_limit::Action::Login, Some([127, 0, 0, 1].into()))
            .await
            .unwrap();
    }
}

async fn serve_once(response: &'static [u8]) -> std::net::SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        stream.write_all(response).await.unwrap();
    });
    addr
}

#[tokio::test]
async fn healthcheck_reports_the_live_endpoint() {
    let healthy = serve_once(b"HTTP/1.1 200 OK\r\ncontent-length: 0\r\n\r\n").await;
    app::healthcheck(healthy).await.unwrap();

    let unhealthy = serve_once(b"HTTP/1.1 503 Service Unavailable\r\n\r\n").await;
    assert!(matches!(
        app::healthcheck(unhealthy).await,
        Err(StartupError::Unhealthy(_))
    ));

    let closed = {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        listener.local_addr().unwrap()
    };
    assert!(app::healthcheck(closed).await.is_err());
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn the_maintenance_task_deletes_expired_sessions(pool: PgPool) {
    let app = TestApp::new(pool.clone());
    app.register("alice@example.com").await;

    let clock = ManualClock::starting_now();
    clock.advance(time::Duration::days(31));
    let services = Arc::new(Services::new(Context::<TestAdapters> {
        db: PostgresDatabase::new(pool.clone()),
        hasher: Argon2Hasher::new(Argon2Params {
            memory_kib: 64,
            iterations: 1,
            parallelism: 1,
            max_concurrent: 1,
            max_queued: 64,
        })
        .unwrap(),
        tokens: RandomTokens,
        crypto: infrastructure::crypto::RingCrypto::new(&[7; 32]),
        clock,
        mailer: Arc::new(RecordingMailer::new()),
        texts: Arc::new(infrastructure::testing::RecordingTexts::new()),
        identity_providers: Arc::new(infrastructure::testing::FakeIdentityProviders::new()),
        translator: Arc::new(i18n::Catalog::embedded().unwrap()),
        settings: application::Settings {
            app_name: "Acme".to_owned(),
            sessions: domain::session::SessionPolicy::default(),
            tokens: domain::user_token::TokenPolicy::default(),
            require_email_verification: false,
            unverified_account_ttl: None,
            audit_retention: None,
            links: Links::new("http://localhost:5173"),
            text_countries: Vec::new(),
            locale: domain::i18n::Locale::EN,
        },
    }));

    let task = tokio::spawn(jobs::maintenance(
        services,
        Arc::new(RateLimits::disabled()),
        Some(infrastructure::db::ClusterLock::new(
            pool.clone(),
            infrastructure::db::ClusterLock::MAINTENANCE,
        )),
    ));
    let mut remaining = 1;
    for _ in 0..50 {
        remaining = sqlx::query_scalar::<_, i64>("select count(*) from sessions")
            .fetch_one(&pool)
            .await
            .unwrap();
        if remaining == 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    task.abort();
    assert_eq!(remaining, 0);
}
