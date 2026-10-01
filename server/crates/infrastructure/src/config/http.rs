//! `BIND_ADDRESS`, `APP_URL` and the other HTTP settings: cookies, proxy trust, HSTS, CORS,
//! timeouts, body limit and rate limits.
//!
//! `APP_URL` decides whether the server counts as local (`localhost`, `127.0.0.1`, `[::1]`); off
//! localhost it must be `https://` and `COOKIE_SECURE` must stay on, and the other sections
//! refuse development-only settings.

use std::{
    fmt::{self, Display, Formatter},
    net::SocketAddr,
    num::NonZeroU32,
    path::PathBuf,
    sync::Arc,
    time::Duration,
};

use super::reader::{Reader, parse_duration};
use crate::rate_limit::{DEFAULT_MEMORY_MAX_KEYS, Rate, Rates};

const HSTS_DEFAULT: Duration = Duration::from_hours(365 * 24);

const MAX_RATE_PERIOD: Duration = Duration::from_hours(7 * 24);

pub(super) const RATE_VARS: &[&str] = &[
    "RATE_LIMIT_API_PER_IP",
    "RATE_LIMIT_LOGIN_PER_IP",
    "RATE_LIMIT_LOGIN_PER_ACCOUNT",
    "RATE_LIMIT_LOGIN_PER_ACCOUNT_TOTAL",
    "RATE_LIMIT_REGISTER_PER_IP",
    "RATE_LIMIT_PASSWORD_RESET_PER_IP",
    "RATE_LIMIT_PASSWORD_RESET_PER_ACCOUNT",
    "RATE_LIMIT_VERIFICATION_PER_IP",
    "RATE_LIMIT_VERIFICATION_PER_ACCOUNT",
    "RATE_LIMIT_SEND_CODE_PER_IP",
    "RATE_LIMIT_SEND_CODE_PER_ACCOUNT",
    "RATE_LIMIT_CHECK_CODE_PER_IP",
    "RATE_LIMIT_CHECK_CODE_PER_ACCOUNT",
    "RATE_LIMIT_CEREMONY_PER_IP",
    "RATE_LIMIT_REPORT_PER_IP",
    "RATE_LIMIT_UPLOAD_PER_IP",
    "RATE_LIMIT_UPLOAD_PER_ACCOUNT",
    "RATE_LIMIT_STORE",
    "RATE_LIMIT_MEMORY_MAX_KEYS",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RateLimitStore {
    #[default]
    Memory,
    Postgres,
}

#[derive(Debug, Clone)]
pub struct HttpConfig {
    pub bind_addr: SocketAddr,
    /// Where browsers open the app. Links in mail point here, and state-changing requests from any
    /// other origin are rejected.
    pub public_url: PublicOrigin,
    /// The built SvelteKit app (`web/build`). When set, it is served for every non-API path, with
    /// `index.html` as the SPA fallback.
    pub static_dir: Option<PathBuf>,
    /// Mark the session cookie `Secure` and give it the `__Host-` prefix. Only turn this off for
    /// development over plain HTTP.
    pub secure_cookies: bool,
    /// How many reverse proxies in front of the app append to `X-Forwarded-For` (`TRUST_PROXY`,
    /// `TRUSTED_PROXY_HOPS`). The client IP is the entry that many places from the end. `0` (the
    /// default) ignores the header: without a proxy that sets it, clients could pick their own IP.
    pub trusted_proxy_hops: usize,
    /// `Strict-Transport-Security` max-age (always with `includeSubDomains`). `None` sends no HSTS
    /// header. Defaults to a year for an `https://` `APP_URL`.
    pub hsts_max_age: Option<Duration>,
    /// Origins allowed to call the API cross-origin with credentials. Empty (the default) sends no
    /// CORS headers at all: the SPA is served same-origin.
    pub cors_origins: Vec<PublicOrigin>,
    pub request_timeout: Duration,
    /// How long a file upload may take (`UPLOAD_TIMEOUT`), instead of `request_timeout`: its body
    /// is the file.
    pub upload_timeout: Duration,
    pub max_body_bytes: usize,
    pub rate_limits: bool,
    pub rates: Rates,
    pub rate_limit_store: RateLimitStore,
    /// How many buckets the memory store keeps at most (`RATE_LIMIT_MEMORY_MAX_KEYS`).
    pub rate_limit_memory_max_keys: usize,
    /// How long a graceful shutdown may take in all: in-flight requests, then queued mail, then
    /// closing the database. Keep it below the orchestrator's grace period (Docker's
    /// `stop_grace_period`, Kubernetes' `terminationGracePeriodSeconds`).
    pub shutdown_timeout: Duration,
}

impl Reader<'_> {
    pub(super) fn http(&mut self) -> Option<HttpConfig> {
        let bind_addr = self.parse(
            "BIND_ADDRESS",
            SocketAddr::from(([127, 0, 0, 1], 3000)),
            |raw| {
                raw.parse()
                    .map_err(|_| "must be an IP address and port, e.g. 127.0.0.1:3000".to_owned())
            },
        );
        let public_url = self.required("APP_URL", PublicOrigin::parse);
        // Without a valid `APP_URL` there is already a problem; do not pile more on top.
        self.local = public_url.as_ref().is_none_or(PublicOrigin::is_localhost);
        let static_dir = self.raw("STATIC_DIR").map(PathBuf::from);
        let secure_cookies = self.bool("COOKIE_SECURE", true);
        let trust_proxy = self.bool("TRUST_PROXY", false);
        let hops: usize = self.number("TRUSTED_PROXY_HOPS", 1);
        if self.raw("TRUSTED_PROXY_HOPS").is_some() && !trust_proxy {
            self.problem("TRUSTED_PROXY_HOPS", "only applies with TRUST_PROXY=true");
        }
        if trust_proxy && hops == 0 {
            self.problem("TRUSTED_PROXY_HOPS", "must be at least 1");
        }
        let https = public_url.as_ref().is_some_and(PublicOrigin::is_https);
        let hsts_default = if https { HSTS_DEFAULT } else { Duration::ZERO };
        let hsts_max_age = Some(self.duration("HSTS_MAX_AGE", hsts_default, 2 * HSTS_DEFAULT))
            .filter(|max_age| !max_age.is_zero());
        let cors_origins = self.parse("CORS_ALLOWED_ORIGINS", Vec::new(), |raw| {
            raw.split(',')
                .map(str::trim)
                .filter(|origin| !origin.is_empty())
                .map(PublicOrigin::parse)
                .collect()
        });
        let request_timeout = self.duration(
            "REQUEST_TIMEOUT",
            Duration::from_secs(30),
            Duration::from_hours(1),
        );
        let max_body_bytes = self.number("MAX_BODY_BYTES", 64 * 1024);
        let rate_limits = self.bool("RATE_LIMITS_ENABLED", true);
        let rate_limit_store =
            self.parse(
                "RATE_LIMIT_STORE",
                RateLimitStore::Memory,
                |raw| match raw {
                    "memory" => Ok(RateLimitStore::Memory),
                    "postgres" => Ok(RateLimitStore::Postgres),
                    _ => Err("must be `memory` or `postgres`".to_owned()),
                },
            );
        let rate_limit_memory_max_keys =
            self.number("RATE_LIMIT_MEMORY_MAX_KEYS", DEFAULT_MEMORY_MAX_KEYS);
        if rate_limit_memory_max_keys == 0 {
            self.problem("RATE_LIMIT_MEMORY_MAX_KEYS", "must be greater than zero");
        }
        let (rates, upload_timeout) = (self.rates(), self.upload_timeout());
        let shutdown_timeout = self.duration(
            "SHUTDOWN_TIMEOUT",
            Duration::from_secs(20),
            Duration::from_mins(10),
        );

        if request_timeout.is_zero() {
            self.problem("REQUEST_TIMEOUT", "must be greater than zero");
        }
        if shutdown_timeout.is_zero() {
            self.problem("SHUTDOWN_TIMEOUT", "must be greater than zero");
        }
        // Off localhost, the session must travel over TLS only, in a `__Host-` cookie no subdomain
        // can plant.
        if !self.local {
            if !https {
                self.problem("APP_URL", "must be https:// unless it is localhost");
            }
            if !secure_cookies {
                self.problem(
                    "COOKIE_SECURE",
                    "must be true unless APP_URL is localhost: without it the session cookie \
                     is sent over plain http and loses its __Host- protection",
                );
            }
        }
        if let Some(host) = public_url.as_ref().and_then(PublicOrigin::ip_literal) {
            self.warnings.push(format!(
                "APP_URL uses the IP address {host}: passkeys need a domain name, so they will \
                 not work"
            ));
        }

        Some(HttpConfig {
            bind_addr,
            public_url: public_url?,
            static_dir,
            secure_cookies,
            trusted_proxy_hops: if trust_proxy { hops } else { 0 },
            hsts_max_age,
            cors_origins,
            request_timeout,
            upload_timeout,
            max_body_bytes,
            rate_limits,
            rates,
            rate_limit_store,
            rate_limit_memory_max_keys,
            shutdown_timeout,
        })
    }
}

impl Reader<'_> {
    /// At most an hour: an unfinished upload's contents are removed some hours later
    /// (`application::files::UNFINISHED_UPLOAD_TTL`), which must not catch a live one.
    fn upload_timeout(&mut self) -> Duration {
        let timeout = self.duration(
            "UPLOAD_TIMEOUT",
            Duration::from_mins(10),
            Duration::from_hours(1),
        );
        if timeout.is_zero() {
            self.problem("UPLOAD_TIMEOUT", "must be greater than zero");
        }
        timeout
    }

    fn rates(&mut self) -> Rates {
        let default = Rates::default();
        Rates {
            api_per_ip: self.parse("RATE_LIMIT_API_PER_IP", default.api_per_ip, |raw| {
                if raw == "off" {
                    Ok(None)
                } else {
                    parse_rate(raw).map(Some)
                }
            }),
            login_per_ip: self.rate("RATE_LIMIT_LOGIN_PER_IP", default.login_per_ip),
            login_per_account: self.rate("RATE_LIMIT_LOGIN_PER_ACCOUNT", default.login_per_account),
            login_per_account_total: self.rate(
                "RATE_LIMIT_LOGIN_PER_ACCOUNT_TOTAL",
                default.login_per_account_total,
            ),
            register_per_ip: self.rate("RATE_LIMIT_REGISTER_PER_IP", default.register_per_ip),
            password_reset_per_ip: self.rate(
                "RATE_LIMIT_PASSWORD_RESET_PER_IP",
                default.password_reset_per_ip,
            ),
            password_reset_per_account: self.rate(
                "RATE_LIMIT_PASSWORD_RESET_PER_ACCOUNT",
                default.password_reset_per_account,
            ),
            verification_per_ip: self.rate(
                "RATE_LIMIT_VERIFICATION_PER_IP",
                default.verification_per_ip,
            ),
            verification_per_account: self.rate(
                "RATE_LIMIT_VERIFICATION_PER_ACCOUNT",
                default.verification_per_account,
            ),
            send_code_per_ip: self.rate("RATE_LIMIT_SEND_CODE_PER_IP", default.send_code_per_ip),
            send_code_per_account: self.rate(
                "RATE_LIMIT_SEND_CODE_PER_ACCOUNT",
                default.send_code_per_account,
            ),
            check_code_per_ip: self.rate("RATE_LIMIT_CHECK_CODE_PER_IP", default.check_code_per_ip),
            check_code_per_account: self.rate(
                "RATE_LIMIT_CHECK_CODE_PER_ACCOUNT",
                default.check_code_per_account,
            ),
            ceremony_per_ip: self.rate("RATE_LIMIT_CEREMONY_PER_IP", default.ceremony_per_ip),
            report_per_ip: self.rate("RATE_LIMIT_REPORT_PER_IP", default.report_per_ip),
            upload_per_ip: self.rate("RATE_LIMIT_UPLOAD_PER_IP", default.upload_per_ip),
            upload_per_account: self
                .rate("RATE_LIMIT_UPLOAD_PER_ACCOUNT", default.upload_per_account),
        }
    }

    fn rate(&mut self, name: &'static str, default: Rate) -> Rate {
        self.parse(name, default, parse_rate)
    }
}

/// Parses `<burst>/<period>`: `20/1m` allows 20 requests at once, then one every 3s.
fn parse_rate(raw: &str) -> Result<Rate, String> {
    const ERROR: &str = "must be `<requests>/<period>`, e.g. `20/1m` or `5/1h`";

    let (burst, period) = raw.split_once('/').ok_or_else(|| ERROR.to_owned())?;
    let burst: NonZeroU32 = burst.trim().parse().map_err(|_| ERROR.to_owned())?;
    let period = parse_duration(period.trim()).map_err(|_| ERROR.to_owned())?;
    if period.is_zero() || period > MAX_RATE_PERIOD {
        return Err("the period must be between 1s and 7d".to_owned());
    }
    Ok(Rate::new(burst, period))
}

/// A scheme, host and optional port, exactly as browsers send it in the `Origin` header:
/// `https://example.com`, `http://localhost:5173`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicOrigin(Arc<str>);

impl PublicOrigin {
    pub fn parse(raw: &str) -> Result<Self, String> {
        const ERROR: &str =
            "must be an http:// or https:// origin without a path, e.g. https://example.com";

        let origin = raw.trim().trim_end_matches('/').to_ascii_lowercase();
        let (scheme, authority) = origin.split_once("://").ok_or_else(|| ERROR.to_owned())?;
        let valid = matches!(scheme, "http" | "https")
            && !authority.is_empty()
            && !authority.contains(['/', '?', '#', '@', ' ', '\\']);
        if !valid {
            return Err(ERROR.to_owned());
        }
        // Browsers leave the scheme's default port out of `Origin` and WebAuthn client data, so
        // `https://example.com:443` must compare equal to `https://example.com`.
        let default_port = if scheme == "https" { ":443" } else { ":80" };
        let authority = authority.strip_suffix(default_port).unwrap_or(authority);
        Ok(Self(format!("{scheme}://{authority}").into()))
    }

    /// The host, if it is an IP address rather than a name. WebAuthn refuses IP addresses as
    /// relying party ids.
    pub fn ip_literal(&self) -> Option<&str> {
        let host = self.host();
        let bare = host.trim_start_matches('[').trim_end_matches(']');
        (bare.parse::<std::net::IpAddr>().is_ok() && !self.is_localhost()).then_some(host)
    }

    fn host(&self) -> &str {
        let authority = self
            .0
            .split_once("://")
            .map_or("", |(_, authority)| authority);
        if authority.starts_with('[') {
            return authority
                .split_once(']')
                .map_or(authority, |(host, _)| &authority[..=host.len()]);
        }
        authority
            .rsplit_once(':')
            .map_or(authority, |(host, _)| host)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_https(&self) -> bool {
        self.0.starts_with("https://")
    }

    /// Browsers treat `http://localhost` as a secure context, so `Secure` cookies work there
    /// without TLS.
    pub fn is_localhost(&self) -> bool {
        matches!(self.host(), "localhost" | "127.0.0.1" | "[::1]")
    }
}

impl Display for PublicOrigin {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
