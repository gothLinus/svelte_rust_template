//! Rate limits: one per client IP on the whole API, and tighter ones per client IP and per account
//! on the authentication endpoints.
//!
//! The buckets live in a [`BucketStore`] (`RATE_LIMIT_STORE`): in memory by default, so limits
//! apply per server process, or in Postgres, shared by every instance behind a load balancer.
//!
//! Keys are canonical, so one account cannot be given several budgets by spelling it differently:
//! identifiers are parsed like the sign-in form parses them (`+49 170…` and `+49(170)…` are one
//! number), IPv6 clients are grouped by their /64, and unparseable input is capped in length
//! before it becomes a key. They are stored as keyed digests, so neither the Postgres table nor a
//! memory dump holds the addresses and numbers.

use std::{
    net::{IpAddr, Ipv6Addr},
    sync::Arc,
};

use application::AppError;
use domain::{
    security::Crypto,
    user::{LoginIdentifier, UserId},
};
pub use infrastructure::rate_limit::{
    BucketStore, DEFAULT_MEMORY_MAX_KEYS, MemoryBuckets, Rate, Rates,
};
use infrastructure::{crypto::RingCrypto, rate_limit::Limited};

use crate::problem::ApiError;

/// What a request is counted as. Each maps to a per-IP bucket; the actions in
/// [`RateLimits::check_account`] and [`RateLimits::check_user`] also have a per-account one. The
/// rates are set by the `RATE_LIMIT_*` variables.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Api,
    Login,
    Register,
    PasswordReset,
    Verification,
    SendCode,
    CheckCode,
    Ceremony,
    Report,
    Upload,
}

const MAX_RAW_KEY_CHARS: usize = 64;

/// The bucket key for something a client named an account by: the canonical form of an email
/// address, username or phone number, so that every spelling of one account shares one bucket.
/// Anything else is lowercased and cut to a bounded length.
pub fn account_key(raw: &str) -> String {
    match LoginIdentifier::parse(raw) {
        Ok(LoginIdentifier::Email(email)) => format!("email:{}", email.as_str()),
        Ok(LoginIdentifier::Username(username)) => format!("user:{}", username.as_str()),
        Ok(LoginIdentifier::Phone(phone)) => format!("phone:{}", phone.as_str()),
        Err(_) => {
            let raw: String = raw
                .trim()
                .to_lowercase()
                .chars()
                .take(MAX_RAW_KEY_CHARS)
                .collect();
            format!("raw:{raw}")
        }
    }
}

pub fn user_key(user: UserId) -> String {
    format!("id:{user}")
}

/// The network a client IP stands for: IPv4 addresses as they are, IPv4-mapped IPv6 addresses as
/// IPv4, and other IPv6 addresses as their /64, the smallest block a provider hands out, so that
/// rotating through a /64 does not buy fresh buckets.
pub fn ip_bucket(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V4(_) => ip,
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or_else(
            || {
                let [a, b, c, d, ..] = v6.segments();
                IpAddr::V6(Ipv6Addr::new(a, b, c, d, 0, 0, 0, 0))
            },
            IpAddr::V4,
        ),
    }
}

/// The coarser network the API-wide limit counts: IPv6 addresses as their /56, the block a
/// provider typically hands one customer, so rotating through the /64s of it does not multiply
/// the budget. IPv4 as [`ip_bucket`].
pub fn api_ip_bucket(ip: IpAddr) -> IpAddr {
    match ip_bucket(ip) {
        IpAddr::V6(v6) => {
            let [a, b, c, d, ..] = v6.segments();
            IpAddr::V6(Ipv6Addr::new(a, b, c, d & 0xff00, 0, 0, 0, 0))
        }
        v4 @ IpAddr::V4(_) => v4,
    }
}

/// A password sign-in that passed the limits. Pass it to [`RateLimits::login_succeeded`] if the
/// password was right, so the attempt is not held against the account.
#[must_use = "a successful sign-in must be given back with `login_succeeded`"]
#[derive(Debug)]
pub struct LoginAttempt {
    account: String,
    ip: Option<IpAddr>,
    counted_total: bool,
}

#[derive(Debug, Clone, Copy)]
enum Bucket {
    ApiIp,
    LoginIp,
    LoginAccount,
    LoginAccountTotal,
    RegisterIp,
    ResetIp,
    ResetAccount,
    VerificationIp,
    VerificationAccount,
    SendCodeIp,
    SendCodeAccount,
    CheckCodeIp,
    CheckCodeAccount,
    CeremonyIp,
    ReportIp,
    UploadIp,
    UploadAccount,
}

impl Bucket {
    const fn name(self) -> &'static str {
        match self {
            Self::ApiIp => "api_ip",
            Self::LoginIp => "login_ip",
            Self::LoginAccount => "login_account",
            Self::LoginAccountTotal => "login_account_total",
            Self::RegisterIp => "register_ip",
            Self::ResetIp => "reset_ip",
            Self::ResetAccount => "reset_account",
            Self::VerificationIp => "verification_ip",
            Self::VerificationAccount => "verification_account",
            Self::SendCodeIp => "send_code_ip",
            Self::SendCodeAccount => "send_code_account",
            Self::CheckCodeIp => "check_code_ip",
            Self::CheckCodeAccount => "check_code_account",
            Self::CeremonyIp => "ceremony_ip",
            Self::ReportIp => "report_ip",
            Self::UploadIp => "upload_ip",
            Self::UploadAccount => "upload_account",
        }
    }

    const fn rate(self, rates: &Rates) -> Option<Rate> {
        let rate = match self {
            Self::ApiIp => return rates.api_per_ip,
            Self::LoginIp => rates.login_per_ip,
            Self::LoginAccount => rates.login_per_account,
            Self::LoginAccountTotal => rates.login_per_account_total,
            Self::RegisterIp => rates.register_per_ip,
            Self::ResetIp => rates.password_reset_per_ip,
            Self::ResetAccount => rates.password_reset_per_account,
            Self::VerificationIp => rates.verification_per_ip,
            Self::VerificationAccount => rates.verification_per_account,
            Self::SendCodeIp => rates.send_code_per_ip,
            Self::SendCodeAccount => rates.send_code_per_account,
            Self::CheckCodeIp => rates.check_code_per_ip,
            Self::CheckCodeAccount => rates.check_code_per_account,
            Self::CeremonyIp => rates.ceremony_per_ip,
            Self::ReportIp => rates.report_per_ip,
            Self::UploadIp => rates.upload_per_ip,
            Self::UploadAccount => rates.upload_per_account,
        };
        Some(rate)
    }
}

fn account_from_ip(account: &str, ip: Option<IpAddr>) -> String {
    match ip {
        Some(ip) => format!("{account}@{ip}"),
        None => account.to_owned(),
    }
}

/// The API's rate limits: per-IP checks (`check_ip`), per-account and per-user checks
/// (`check_account`, `check_user`), and the accounting of password sign-ins (`start_login`).
/// Every check fails with a `429` [`ApiError`] when a bucket is empty. A store failure fails the
/// request too, rather than skipping the limit.
pub struct RateLimits {
    enabled: bool,
    rates: Rates,
    store: BucketStore,
    keys: Arc<dyn Crypto>,
}

impl RateLimits {
    /// `keys` digests the bucket keys (the server's `SECRET_KEY`, so every instance sharing a
    /// Postgres store derives the same ones).
    pub fn new(rates: Rates, store: BucketStore, keys: Arc<dyn Crypto>) -> Self {
        Self {
            enabled: true,
            rates,
            store,
            keys,
        }
    }

    pub fn in_memory(rates: Rates) -> Self {
        let random = RingCrypto::new(&[0; 32])
            .random_bytes(32)
            .unwrap_or_default();
        let mut key = [0u8; 32];
        key[..random.len()].copy_from_slice(&random);
        Self::new(
            rates,
            BucketStore::memory(),
            Arc::new(RingCrypto::new(&key)),
        )
    }

    fn stored_key(&self, key: &str) -> String {
        let digest = self.keys.keyed_digest("rate limit keys", key.as_bytes());
        digest[..16]
            .iter()
            .fold(String::with_capacity(32), |mut hex, byte| {
                use std::fmt::Write as _;
                let _infallible = write!(hex, "{byte:02x}");
                hex
            })
    }

    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Self::in_memory(Rates::default())
        }
    }

    async fn take(&self, bucket: Bucket, key: &str) -> Result<(), ApiError> {
        let Some(rate) = bucket.rate(&self.rates) else {
            return Ok(());
        };
        match self
            .store
            .take(bucket.name(), &self.stored_key(key), rate)
            .await
        {
            Ok(Ok(())) => Ok(()),
            Ok(Err(Limited { retry_after })) => Err(ApiError::rate_limited(retry_after)),
            Err(err) => Err(AppError::from(err).into()),
        }
    }

    /// Returns a request, logging a failure: the request it belongs to succeeded, and it only costs
    /// the account one attempt of its budget.
    async fn give_back(&self, bucket: Bucket, key: &str) {
        let Some(rate) = bucket.rate(&self.rates) else {
            return;
        };
        if let Err(err) = self
            .store
            .give_back(bucket.name(), &self.stored_key(key), rate)
            .await
        {
            ApiError::log_detached(err.into());
        }
    }

    /// Counts a request from `ip` against `action`. Requests without a known IP, which is only
    /// possible when the server was not started with connection info such as in tests, are not
    /// limited per IP.
    pub async fn check_ip(&self, action: Action, ip: Option<IpAddr>) -> Result<(), ApiError> {
        let Some(ip) = ip.filter(|_| self.enabled) else {
            return Ok(());
        };
        let ip = match action {
            Action::Api => api_ip_bucket(ip),
            _ => ip_bucket(ip),
        };
        let bucket = match action {
            Action::Api => Bucket::ApiIp,
            Action::Login => Bucket::LoginIp,
            Action::Register => Bucket::RegisterIp,
            Action::PasswordReset => Bucket::ResetIp,
            Action::Verification => Bucket::VerificationIp,
            Action::SendCode => Bucket::SendCodeIp,
            Action::CheckCode => Bucket::CheckCodeIp,
            Action::Ceremony => Bucket::CeremonyIp,
            Action::Report => Bucket::ReportIp,
            Action::Upload => Bucket::UploadIp,
        };
        self.take(bucket, &ip.to_string()).await
    }

    /// Counts a request for the account a client named (an email address, username or phone number,
    /// in any spelling) against `action`. Password sign-ins go through [`RateLimits::start_login`]
    /// instead.
    pub async fn check_account(&self, action: Action, account: &str) -> Result<(), ApiError> {
        self.check_key(action, &account_key(account)).await
    }

    pub async fn check_user(&self, action: Action, user: UserId) -> Result<(), ApiError> {
        self.check_key(action, &user_key(user)).await
    }

    async fn check_key(&self, action: Action, key: &str) -> Result<(), ApiError> {
        if !self.enabled {
            return Ok(());
        }
        let bucket = match action {
            Action::Login => Bucket::LoginAccountTotal,
            Action::PasswordReset => Bucket::ResetAccount,
            Action::Verification => Bucket::VerificationAccount,
            Action::SendCode => Bucket::SendCodeAccount,
            Action::CheckCode => Bucket::CheckCodeAccount,
            Action::Upload => Bucket::UploadAccount,
            Action::Api | Action::Register | Action::Ceremony | Action::Report => {
                return Ok(());
            }
        };
        self.take(bucket, key).await
    }

    /// Counts a password sign-in for `account` (the key from [`account_key`], or better
    /// [`user_key`] once the account is known, so its address, username and number share one
    /// budget) against the tight limit for this account from this IP and against the account's
    /// overall ceiling. The per-IP limit is [`RateLimits::check_ip`]'s. Give the attempt back with
    /// [`RateLimits::login_succeeded`] if the password was right: only failures count.
    ///
    /// A `known_device` (a browser the owner signed in on before) skips the ceiling: strangers
    /// exhausting it from many IPs would otherwise lock the owner out of password sign-in
    /// everywhere. It still gets the per-IP limits.
    pub async fn start_login(
        &self,
        account: String,
        ip: Option<IpAddr>,
        known_device: bool,
    ) -> Result<LoginAttempt, ApiError> {
        let ip = ip.map(ip_bucket);
        let attempt = LoginAttempt {
            account,
            ip,
            counted_total: !known_device,
        };
        if !self.enabled {
            return Ok(attempt);
        }
        let from_ip = account_from_ip(&attempt.account, attempt.ip);
        self.take(Bucket::LoginAccount, &from_ip).await?;
        if known_device {
            return Ok(attempt);
        }
        if let Err(limited) = self.take(Bucket::LoginAccountTotal, &attempt.account).await {
            self.give_back(Bucket::LoginAccount, &from_ip).await;
            return Err(limited);
        }
        Ok(attempt)
    }

    pub async fn login_succeeded(&self, attempt: LoginAttempt) {
        if !self.enabled {
            return;
        }
        if attempt.counted_total {
            self.give_back(Bucket::LoginAccountTotal, &attempt.account)
                .await;
        }
        self.give_back(
            Bucket::LoginAccount,
            &account_from_ip(&attempt.account, attempt.ip),
        )
        .await;
    }

    pub async fn retain_recent(&self) -> Result<u64, AppError> {
        Ok(self.store.retain_recent().await?)
    }
}
