use std::{collections::HashMap, time::Duration};

use infrastructure::{
    config::{
        ClientSecret, Config, ConfigError, LogFormat, MailTransport, PublicOrigin, RateLimitStore,
        SecretKey, TextTransport, parse_duration, unknown_variables,
    },
    oauth::Provider,
};

const KEY: &str = "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=";
const OTHER_KEY: &str = "ICEiIyQlJicoKSorLC0uLzAxMjM0NTY3ODk6Ozw9Pj8=";

const MINIMAL: &[(&str, &str)] = &[
    ("APP_URL", "http://localhost:5173"),
    ("DATABASE_URL", "postgres://app:app@localhost:5432/app"),
    ("MAIL_FROM", "Example <noreply@example.com>"),
    ("SECRET_KEY", KEY),
];

fn load(vars: &[(&str, &str)]) -> Result<Config, ConfigError> {
    let map: HashMap<String, String> = MINIMAL
        .iter()
        .chain(vars)
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect();
    Config::from_lookup(|name| map.get(name).cloned())
}

fn problems(vars: &[(&str, &str)]) -> Vec<&'static str> {
    load(vars)
        .unwrap_err()
        .0
        .into_iter()
        .map(|problem| problem.variable)
        .collect()
}

#[test]
fn minimal_config_uses_defaults() {
    let config = load(&[]).unwrap();

    assert_eq!(config.http.bind_addr.to_string(), "127.0.0.1:3000");
    assert_eq!(config.http.public_url.as_str(), "http://localhost:5173");
    assert!(config.http.secure_cookies);
    assert_eq!(config.http.trusted_proxy_hops, 0);
    assert!(config.http.hsts_max_age.is_none());
    assert_eq!(config.http.shutdown_timeout, Duration::from_secs(20));
    assert!(config.texts.allowed_countries.is_empty());
    assert!(config.warnings.is_empty());
    assert!(config.http.cors_origins.is_empty());
    assert!(config.http.rate_limits);
    assert_eq!(config.http.rate_limit_store, RateLimitStore::Memory);
    assert_eq!(config.database.statement_timeout, Duration::from_secs(30));
    assert_eq!(config.auth.argon2.max_queued, 64);
    assert_eq!(config.http.request_timeout, Duration::from_secs(30));
    assert_eq!(config.database.max_connections, 10);
    assert!(config.database.run_migrations);
    assert!(matches!(config.mail.transport, MailTransport::Log));
    assert!(!config.auth.require_email_verification);
    assert_eq!(config.auth.sessions.idle_timeout, time::Duration::days(7));
    assert_eq!(config.log_format, LogFormat::Text);
    assert_eq!(config.auth.app_name, "Acme");
    assert_eq!(config.auth.secret_key.bytes()[31], 31);
    assert!(matches!(config.texts.transport, TextTransport::Disabled));
    assert!(config.oauth.providers.is_empty());
}

#[test]
fn every_variable_is_read() {
    let config = load(&[
        ("BIND_ADDRESS", "0.0.0.0:8080"),
        ("APP_URL", "https://app.example.com/"),
        ("STATIC_DIR", "../web/build"),
        ("COOKIE_SECURE", "true"),
        ("TRUST_PROXY", "true"),
        ("HSTS_MAX_AGE", "365d"),
        (
            "CORS_ALLOWED_ORIGINS",
            "https://a.example.com, https://b.example.com",
        ),
        ("REQUEST_TIMEOUT", "10s"),
        ("MAX_BODY_BYTES", "1024"),
        ("RATE_LIMITS_ENABLED", "false"),
        ("RATE_LIMIT_STORE", "postgres"),
        ("DATABASE_STATEMENT_TIMEOUT", "0"),
        ("DATABASE_MAX_CONNECTIONS", "3"),
        ("DATABASE_ACQUIRE_TIMEOUT", "2s"),
        ("DATABASE_RUN_MIGRATIONS", "false"),
        ("MAIL_TRANSPORT", "smtp"),
        ("SMTP_URL", "smtp://localhost:1025"),
        ("TRUSTED_PROXY_HOPS", "2"),
        ("SHUTDOWN_TIMEOUT", "25s"),
        ("TEXT_ALLOWED_COUNTRIES", "+49, +43"),
        ("SECRET_KEY_PREVIOUS", OTHER_KEY),
        ("SESSION_IDLE_TIMEOUT", "1h"),
        ("SESSION_LIFETIME", "1d"),
        ("EMAIL_VERIFICATION_TTL", "2h"),
        ("PASSWORD_RESET_TTL", "15m"),
        ("REQUIRE_EMAIL_VERIFICATION", "true"),
        ("ARGON2_MEMORY_KIB", "8192"),
        ("ARGON2_ITERATIONS", "3"),
        ("ARGON2_PARALLELISM", "2"),
        ("ARGON2_MAX_CONCURRENT", "8"),
        ("ARGON2_MAX_QUEUED", "0"),
        ("LOG_FORMAT", "json"),
    ])
    .unwrap();

    assert_eq!(config.http.bind_addr.port(), 8080);
    assert_eq!(config.http.public_url.as_str(), "https://app.example.com");
    assert!(config.http.static_dir.is_some());
    assert_eq!(config.http.trusted_proxy_hops, 2);
    assert_eq!(config.http.shutdown_timeout, Duration::from_secs(25));
    let countries: Vec<&str> = config
        .texts
        .allowed_countries
        .iter()
        .map(domain::user::CallingCode::as_str)
        .collect();
    assert_eq!(countries, ["+49", "+43"]);
    assert!(config.auth.previous_secret_key.is_some());
    assert_eq!(
        config.http.hsts_max_age,
        Some(Duration::from_hours(365 * 24))
    );
    assert_eq!(config.http.cors_origins.len(), 2);
    assert_eq!(config.http.max_body_bytes, 1024);
    assert!(!config.http.rate_limits);
    assert_eq!(config.http.rate_limit_store, RateLimitStore::Postgres);
    assert_eq!(config.database.max_connections, 3);
    assert!(config.database.statement_timeout.is_zero());
    assert_eq!(config.auth.argon2.max_queued, 0);
    assert!(!config.database.run_migrations);
    assert!(matches!(config.mail.transport, MailTransport::Smtp { .. }));
    assert_eq!(config.auth.sessions.idle_timeout, time::Duration::hours(1));
    assert_eq!(
        config.auth.tokens.password_reset_ttl,
        time::Duration::minutes(15)
    );
    assert!(config.auth.require_email_verification);
    assert_eq!(config.auth.argon2.memory_kib, 8192);
    assert_eq!(config.auth.argon2.max_concurrent, 8);
    assert_eq!(config.log_format, LogFormat::Json);
}

#[test]
fn all_problems_are_reported_at_once() {
    let map: HashMap<&str, &str> = [
        ("BIND_ADDRESS", "nope"),
        ("COOKIE_SECURE", "maybe"),
        ("LOG_FORMAT", "xml"),
    ]
    .into();
    let err = Config::from_lookup(|name| map.get(name).map(|v| (*v).to_owned())).unwrap_err();
    let variables: Vec<&str> = err.0.iter().map(|p| p.variable).collect();

    for expected in [
        "BIND_ADDRESS",
        "APP_URL",
        "COOKIE_SECURE",
        "DATABASE_URL",
        "MAIL_FROM",
        "SECRET_KEY",
        "LOG_FORMAT",
    ] {
        assert!(
            variables.contains(&expected),
            "{expected} missing from {variables:?}"
        );
    }
    let message = err.to_string();
    assert!(message.starts_with("invalid configuration:"));
    assert!(message.contains("APP_URL: is required"));
}

#[test]
fn invalid_values_are_rejected() {
    assert_eq!(problems(&[("APP_URL", "localhost:5173")]), ["APP_URL"]);
    assert_eq!(problems(&[("DATABASE_URL", "mysql://x")]), ["DATABASE_URL"]);
    assert_eq!(problems(&[("MAIL_FROM", "not a mailbox")]), ["MAIL_FROM"]);
    assert_eq!(
        problems(&[("MAIL_TRANSPORT", "carrier-pigeon")]),
        ["MAIL_TRANSPORT"]
    );
    assert_eq!(problems(&[("MAIL_TRANSPORT", "smtp")]), ["SMTP_URL"]);
    assert_eq!(problems(&[("REQUEST_TIMEOUT", "0")]), ["REQUEST_TIMEOUT"]);
    assert_eq!(
        problems(&[("DATABASE_MAX_CONNECTIONS", "0")]),
        ["DATABASE_MAX_CONNECTIONS"]
    );
    assert_eq!(
        problems(&[("SESSION_IDLE_TIMEOUT", "40d")]),
        ["SESSION_IDLE_TIMEOUT"]
    );
    assert_eq!(
        problems(&[("ARGON2_MEMORY_KIB", "1")]),
        ["ARGON2_MEMORY_KIB"]
    );
    assert_eq!(
        problems(&[("CORS_ALLOWED_ORIGINS", "https://ok.example.com,bad")]),
        ["CORS_ALLOWED_ORIGINS"]
    );
    assert_eq!(problems(&[("MAX_BODY_BYTES", "-1")]), ["MAX_BODY_BYTES"]);
}

#[test]
fn off_localhost_the_session_needs_https_and_secure_cookies() {
    assert_eq!(
        deployed_problems(&[("APP_URL", "http://app.example.com")]),
        ["APP_URL"]
    );
    assert_eq!(
        deployed_problems(&[("COOKIE_SECURE", "false")]),
        ["COOKIE_SECURE"]
    );
    assert_eq!(
        deployed_problems(&[
            ("APP_URL", "http://app.example.com"),
            ("COOKIE_SECURE", "false"),
        ]),
        ["APP_URL", "COOKIE_SECURE"]
    );
    // Browsers treat localhost as a secure context, and development runs over http.
    assert!(load(&[("APP_URL", "http://127.0.0.1:3000")]).is_ok());
    assert!(load(&[("COOKIE_SECURE", "false")]).is_ok());
}

#[test]
fn origins_drop_the_default_port() {
    for (raw, origin) in [
        ("https://example.com:443", "https://example.com"),
        ("http://example.com:80", "http://example.com"),
        ("https://example.com:8443", "https://example.com:8443"),
        ("http://localhost:5173/", "http://localhost:5173"),
        ("https://[2001:db8::1]:443", "https://[2001:db8::1]"),
    ] {
        assert_eq!(PublicOrigin::parse(raw).unwrap().as_str(), origin, "{raw}");
    }
    assert_eq!(
        deployed(&[("APP_URL", "https://app.example.com:443")])
            .unwrap()
            .http
            .public_url
            .as_str(),
        "https://app.example.com"
    );
}

#[test]
fn an_ip_address_as_app_url_is_warned_about() {
    let config = deployed(&[("APP_URL", "https://203.0.113.7")]).unwrap();
    assert_eq!(config.warnings.len(), 1);
    assert!(
        config.warnings[0].contains("passkeys"),
        "{:?}",
        config.warnings
    );
    assert!(
        PublicOrigin::parse("https://[2001:db8::1]")
            .unwrap()
            .ip_literal()
            .is_some()
    );
    assert!(deployed(&[]).unwrap().warnings.is_empty());
    assert!(
        load(&[("APP_URL", "http://127.0.0.1:3000")])
            .unwrap()
            .warnings
            .is_empty()
    );
}

#[test]
fn texting_worldwide_must_be_chosen_explicitly_off_localhost() {
    let twilio = [
        ("TEXT_TRANSPORT", "twilio"),
        ("TWILIO_ACCOUNT_SID", "AC123"),
        ("TWILIO_AUTH_TOKEN", "secret"),
        ("TWILIO_SMS_FROM", "+15550001111"),
    ];
    assert_eq!(deployed_problems(&twilio), ["TEXT_ALLOWED_COUNTRIES"]);

    let listed = [twilio.as_slice(), &[("TEXT_ALLOWED_COUNTRIES", "+41,+49")]].concat();
    let countries = deployed(&listed).unwrap().texts.allowed_countries;
    let countries: Vec<&str> = countries
        .iter()
        .map(domain::user::CallingCode::as_str)
        .collect();
    assert_eq!(countries, ["+41", "+49"]);
    let everywhere = [twilio.as_slice(), &[("TEXT_ALLOWED_COUNTRIES", "*")]].concat();
    assert!(
        deployed(&everywhere)
            .unwrap()
            .texts
            .allowed_countries
            .is_empty()
    );
    assert!(load(&twilio).is_ok());
}

#[test]
fn smtp_credentials_need_tls_off_localhost() {
    assert_eq!(
        deployed_problems(&[("SMTP_URL", "smtp://user:pass@mail.example.com:587")]),
        ["SMTP_URL"]
    );
    for url in [
        "smtps://user:pass@mail.example.com",
        "smtp://user:pass@mail.example.com:587?tls=required",
        "smtp://mail.example.com:25",
    ] {
        assert!(deployed(&[("SMTP_URL", url)]).is_ok(), "{url}");
    }
    assert!(
        load(&[
            ("MAIL_TRANSPORT", "smtp"),
            ("SMTP_URL", "smtp://user:pass@localhost:1025"),
        ])
        .is_ok()
    );
}

#[test]
fn secret_keys_must_look_random_and_rotation_must_change_them() {
    for weak in [
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=",
        "cGFzc3dvcmRwYXNzd29yZHBhc3N3b3JkcGFzc3dvcmQ=",
    ] {
        assert_eq!(problems(&[("SECRET_KEY", weak)]), ["SECRET_KEY"], "{weak}");
    }
    assert_eq!(
        problems(&[("SECRET_KEY_PREVIOUS", KEY)]),
        ["SECRET_KEY_PREVIOUS"]
    );
    assert!(load(&[("SECRET_KEY_PREVIOUS", OTHER_KEY)]).is_ok());
}

#[test]
fn secrets_are_redacted_from_debug() {
    let config = load(&[
        ("MAIL_TRANSPORT", "smtp"),
        ("SMTP_URL", "smtp://user:hunter2@mail.example.com"),
    ])
    .unwrap();
    let debug = format!("{config:?}");
    assert!(!debug.contains("hunter2"));
    assert!(!debug.contains("app:app"));
}

#[test]
fn empty_values_count_as_unset() {
    let config = load(&[("BIND_ADDRESS", "  "), ("LOG_FORMAT", "")]).unwrap();
    assert_eq!(config.http.bind_addr.port(), 3000);
}

#[test]
fn durations() {
    assert_eq!(parse_duration("90").unwrap(), Duration::from_secs(90));
    assert_eq!(parse_duration("90s").unwrap(), Duration::from_secs(90));
    assert_eq!(parse_duration("15m").unwrap(), Duration::from_mins(15));
    assert_eq!(parse_duration("12h").unwrap(), Duration::from_hours(12));
    assert_eq!(parse_duration("7d").unwrap(), Duration::from_hours(7 * 24));
    for bad in ["", "m", "1w", "1.5h", "-1s", "99999999999999999999d"] {
        assert!(parse_duration(bad).is_err(), "{bad}");
    }
}

#[test]
fn public_origins() {
    let origin = PublicOrigin::parse("HTTPS://Example.com:8443/").unwrap();
    assert_eq!(origin.as_str(), "https://example.com:8443");
    assert_eq!(origin.to_string(), "https://example.com:8443");
    assert!(origin.is_https());
    assert!(!origin.is_localhost());
    assert!(
        PublicOrigin::parse("http://localhost:5173")
            .unwrap()
            .is_localhost()
    );
    assert!(
        PublicOrigin::parse("http://[::1]:3000")
            .unwrap()
            .is_localhost()
    );

    for bad in [
        "example.com",
        "ftp://example.com",
        "https://",
        "https://example.com/app",
        "https://user@example.com",
        "https://example.com?x=1",
    ] {
        assert!(PublicOrigin::parse(bad).is_err(), "{bad}");
    }
}

#[test]
fn secret_keys_are_32_bytes_of_base64() {
    assert_eq!(SecretKey::parse(KEY).unwrap().bytes()[1], 1);
    assert_eq!(
        SecretKey::parse(KEY.trim_end_matches('=')).unwrap().bytes()[1],
        1
    );
    assert_eq!(
        SecretKey::parse("AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxz7_78")
            .unwrap()
            .bytes()[31],
        0xbf
    );
    assert!(SecretKey::parse("dG9vIHNob3J0").is_err());
    assert!(SecretKey::parse("not base64!").is_err());
    assert_eq!(problems(&[("SECRET_KEY", "short")]), ["SECRET_KEY"]);
    assert!(!format!("{:?}", SecretKey::parse(KEY).unwrap()).contains("AAEC"));
}

#[test]
fn texting_transports() {
    let config = load(&[("TEXT_TRANSPORT", "log")]).unwrap();
    assert!(matches!(config.texts.transport, TextTransport::Log));

    let config = load(&[
        ("TEXT_TRANSPORT", "twilio"),
        ("TWILIO_ACCOUNT_SID", "AC123"),
        ("TWILIO_AUTH_TOKEN", "secret"),
        ("TWILIO_WHATSAPP_FROM", "+1 555 000 1111"),
    ])
    .unwrap();
    let TextTransport::Twilio(twilio) = config.texts.transport else {
        panic!("expected Twilio")
    };
    assert_eq!(twilio.account_sid, "AC123");
    assert!(twilio.sms_from.is_none());
    assert_eq!(twilio.whatsapp_from.unwrap().as_str(), "+15550001111");

    assert_eq!(
        problems(&[("TEXT_TRANSPORT", "twilio")]),
        ["TWILIO_ACCOUNT_SID", "TWILIO_AUTH_TOKEN", "TWILIO_SMS_FROM"]
    );
    assert_eq!(
        problems(&[("TEXT_TRANSPORT", "pigeon")]),
        ["TEXT_TRANSPORT"]
    );
    assert_eq!(
        problems(&[
            ("TEXT_TRANSPORT", "twilio"),
            ("TWILIO_ACCOUNT_SID", "AC123"),
            ("TWILIO_AUTH_TOKEN", "secret"),
            ("TWILIO_SMS_FROM", "0170"),
        ]),
        ["TWILIO_SMS_FROM", "TWILIO_SMS_FROM"]
    );
}

#[test]
fn social_providers_are_on_when_their_client_id_is_set() {
    let config = load(&[
        ("OAUTH_GITHUB_CLIENT_ID", "gh-id"),
        ("OAUTH_GITHUB_CLIENT_SECRET", "gh-secret"),
        ("OAUTH_MICROSOFT_CLIENT_ID", "ms-id"),
        ("OAUTH_MICROSOFT_CLIENT_SECRET", "ms-secret"),
        ("OAUTH_MICROSOFT_TENANT", "consumers"),
    ])
    .unwrap();
    let providers: Vec<Provider> = config.oauth.providers.iter().map(|p| p.provider).collect();
    assert_eq!(providers, [Provider::Github, Provider::Microsoft]);
    assert_eq!(
        config.oauth.providers[1].microsoft_tenant.as_deref(),
        Some("consumers")
    );
    assert!(matches!(
        config.oauth.providers[0].secret,
        ClientSecret::Shared(_)
    ));
    assert!(!format!("{config:?}").contains("gh-secret"));

    assert_eq!(
        problems(&[("OAUTH_GOOGLE_CLIENT_ID", "id")]),
        ["OAUTH_GOOGLE_CLIENT_SECRET"]
    );
    assert_eq!(
        problems(&[("OAUTH_APPLE_CLIENT_ID", "id")]),
        [
            "OAUTH_APPLE_TEAM_ID",
            "OAUTH_APPLE_KEY_ID",
            "OAUTH_APPLE_PRIVATE_KEY"
        ]
    );
    assert_eq!(
        problems(&[
            ("OAUTH_APPLE_CLIENT_ID", "id"),
            ("OAUTH_APPLE_TEAM_ID", "team"),
            ("OAUTH_APPLE_KEY_ID", "key"),
            (
                "OAUTH_APPLE_PRIVATE_KEY",
                "-----BEGIN PRIVATE KEY-----\\nnope\\n-----END PRIVATE KEY-----"
            ),
        ]),
        ["OAUTH_APPLE_PRIVATE_KEY"]
    );
}

const DEPLOYED: &[(&str, &str)] = &[
    ("APP_URL", "https://app.example.com"),
    ("MAIL_TRANSPORT", "smtp"),
    ("SMTP_URL", "smtp://mail.example.com"),
];

fn deployed(vars: &[(&str, &str)]) -> Result<Config, ConfigError> {
    let all: Vec<(&str, &str)> = DEPLOYED.iter().chain(vars).copied().collect();
    load(&all)
}

fn deployed_problems(vars: &[(&str, &str)]) -> Vec<&'static str> {
    deployed(vars)
        .unwrap_err()
        .0
        .into_iter()
        .map(|problem| problem.variable)
        .collect()
}

#[test]
fn development_shortcuts_are_refused_off_localhost() {
    assert!(deployed(&[]).is_ok());

    // Forgetting the transport would mean mail is silently only logged.
    let unset = load(&[("APP_URL", "https://app.example.com")]).unwrap_err();
    assert_eq!(unset.0[0].variable, "MAIL_TRANSPORT");
    assert_eq!(
        deployed_problems(&[("MAIL_TRANSPORT", "log")]),
        ["MAIL_TRANSPORT"]
    );
    assert_eq!(
        deployed_problems(&[("TEXT_TRANSPORT", "log")]),
        ["TEXT_TRANSPORT"]
    );
    // The key from .env.example seals nothing anyone could not unseal.
    assert_eq!(
        deployed_problems(&[("SECRET_KEY", "ZGV2ZWxvcG1lbnQtb25seS1rZXktY2hhbmdlLW1lISE=")]),
        ["SECRET_KEY"]
    );

    let local = load(&[
        ("SECRET_KEY", "ZGV2ZWxvcG1lbnQtb25seS1rZXktY2hhbmdlLW1lISE="),
        ("TEXT_TRANSPORT", "log"),
    ])
    .unwrap();
    assert!(local.auth.secret_key.is_development_key());
    assert!(matches!(local.mail.transport, MailTransport::Log));
}

#[test]
fn hsts_defaults_to_a_year_over_https() {
    let config = deployed(&[]).unwrap();
    assert_eq!(
        config.http.hsts_max_age,
        Some(Duration::from_hours(365 * 24))
    );
    assert!(
        deployed(&[("HSTS_MAX_AGE", "0")])
            .unwrap()
            .http
            .hsts_max_age
            .is_none()
    );
    assert!(load(&[]).unwrap().http.hsts_max_age.is_none());
}

#[test]
fn durations_are_bounded() {
    // Ten million days would overflow the date arithmetic on the first sign-in.
    assert_eq!(
        problems(&[("SESSION_LIFETIME", "10000000d")]),
        ["SESSION_LIFETIME"]
    );
    assert_eq!(
        problems(&[("PASSWORD_RESET_TTL", "8d")]),
        ["PASSWORD_RESET_TTL"]
    );
    assert_eq!(problems(&[("MAGIC_LINK_TTL", "0")]), ["MAGIC_LINK_TTL"]);
    assert_eq!(
        problems(&[("SHUTDOWN_TIMEOUT", "1h")]),
        ["SHUTDOWN_TIMEOUT"]
    );
    let message = load(&[("SESSION_LIFETIME", "366d")])
        .unwrap_err()
        .to_string();
    assert!(
        message.contains("SESSION_LIFETIME: must be at most 365d"),
        "{message}"
    );
}

#[test]
fn argon2_problems_name_the_right_variable() {
    assert_eq!(
        problems(&[("ARGON2_ITERATIONS", "0")]),
        ["ARGON2_ITERATIONS"]
    );
    assert_eq!(
        problems(&[("ARGON2_PARALLELISM", "0")]),
        ["ARGON2_PARALLELISM"]
    );
    assert_eq!(
        problems(&[("ARGON2_MAX_CONCURRENT", "0")]),
        ["ARGON2_MAX_CONCURRENT"]
    );
}

#[test]
fn unknown_rate_limit_stores_are_rejected() {
    assert_eq!(
        problems(&[("RATE_LIMIT_STORE", "redis")]),
        ["RATE_LIMIT_STORE"]
    );
}

#[test]
fn rate_limits_can_be_overridden() {
    let defaults = load(&[]).unwrap().http.rates;
    assert!(defaults.api_per_ip.is_some());

    let rates = load(&[
        ("RATE_LIMIT_LOGIN_PER_IP", "5/30s"),
        ("RATE_LIMIT_REPORT_PER_IP", " 100 / 1h "),
        ("RATE_LIMIT_API_PER_IP", "off"),
    ])
    .unwrap()
    .http
    .rates;
    assert_eq!(rates.login_per_ip.burst.get(), 5);
    assert_eq!(rates.login_per_ip.period, Duration::from_secs(30));
    assert_eq!(rates.report_per_ip.burst.get(), 100);
    assert_eq!(rates.report_per_ip.period, Duration::from_hours(1));
    assert!(rates.api_per_ip.is_none());
    assert_eq!(rates.register_per_ip, defaults.register_per_ip);
}

#[test]
fn invalid_rate_limits_are_rejected() {
    for bad in ["20", "0/1m", "20/0s", "20/8d", "x/1m", "20/1w", "off"] {
        assert_eq!(
            problems(&[("RATE_LIMIT_LOGIN_PER_IP", bad)]),
            ["RATE_LIMIT_LOGIN_PER_IP"],
            "{bad}"
        );
    }
    assert_eq!(
        problems(&[("RATE_LIMIT_API_PER_IP", "none")]),
        ["RATE_LIMIT_API_PER_IP"]
    );
}

#[test]
fn proxy_hops_need_trust_proxy() {
    assert_eq!(
        problems(&[("TRUSTED_PROXY_HOPS", "2")]),
        ["TRUSTED_PROXY_HOPS"]
    );
    assert_eq!(
        problems(&[("TRUST_PROXY", "true"), ("TRUSTED_PROXY_HOPS", "0")]),
        ["TRUSTED_PROXY_HOPS"]
    );
    let config = load(&[("TRUST_PROXY", "true")]).unwrap();
    assert_eq!(config.http.trusted_proxy_hops, 1);
}

#[test]
fn text_countries_are_calling_codes() {
    assert_eq!(
        problems(&[("TEXT_ALLOWED_COUNTRIES", "+49,DE")]),
        ["TEXT_ALLOWED_COUNTRIES"]
    );
    assert_eq!(
        problems(&[("TEXT_ALLOWED_COUNTRIES", "+0")]),
        ["TEXT_ALLOWED_COUNTRIES"]
    );
}

#[test]
fn misspelled_provider_and_twilio_variables_are_reported() {
    let warnings = unknown_variables(
        [
            "OAUTH_GOOGLE_CLIENTID",
            "OAUTH_GOOGLE_CLIENT_ID",
            "OAUTH_MICROSOFT_TENANT",
            "TWILIO_SMS_FORM",
            "TWILIO_AUTH_TOKEN",
            "TEXT_ALLOWED_COUNTRIES",
            "RATE_LIMIT_LOGIN_PER_IP",
            "RATE_LIMIT_LOGIN_PER_USER",
            "RATE_LIMIT_STORE",
            "RATE_LIMITS_ENABLED",
            "PATH",
        ]
        .map(str::to_owned),
    );
    assert_eq!(warnings.len(), 3, "{warnings:?}");
    assert!(warnings[0].starts_with("OAUTH_GOOGLE_CLIENTID "));
    assert!(warnings[1].starts_with("RATE_LIMIT_LOGIN_PER_USER "));
    assert!(warnings[2].starts_with("TWILIO_SMS_FORM "));
}
