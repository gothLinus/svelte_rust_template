use std::sync::Arc;

use application::{Adapters, Services};
use domain::i18n::Translator;

use crate::{background::Background, cookie::AppCookie, rate_limit::RateLimits};

/// Shared by all requests; cloning it is cheap. Handlers reach the use cases through `services`,
/// and check `limits` themselves where the router's per-IP limit is not enough.
///
/// `cookie` carries the session. The others each carry one step of a flow, so the steps cannot be
/// confused with a session (see [`AppCookie`]): the pending second sign-in step, a social sign-in
/// in progress, a registration awaiting verification, and the mark of a browser the owner signed
/// in on.
pub struct AppState<A: Adapters> {
    pub services: Arc<Services<A>>,
    pub cookie: AppCookie,
    pub mfa_cookie: AppCookie,
    pub oauth_cookie: AppCookie,
    pub registration_cookie: AppCookie,
    pub device_cookie: AppCookie,
    pub limits: Arc<RateLimits>,
    /// The catalog that words errors, in the language of each request; it is the one the use cases
    /// use for mail and texts.
    pub translator: Arc<dyn Translator>,
    /// How many proxies append to `X-Forwarded-For`; `0` ignores the header (see `TRUST_PROXY`).
    pub trusted_proxy_hops: usize,
    pub background: Background,
}

impl<A: Adapters> AppState<A> {
    /// The other cookies are derived from `cookie`'s `Secure` setting and the configured lifetimes;
    /// `background` starts as [`Background::inline`].
    pub fn new(
        services: Arc<Services<A>>,
        cookie: AppCookie,
        limits: Arc<RateLimits>,
        trusted_proxy_hops: usize,
    ) -> Self {
        let verification_ttl = services.context().settings.tokens.email_verification_ttl;
        Self {
            translator: Arc::clone(&services.context().translator),
            mfa_cookie: AppCookie::mfa(cookie.is_secure()),
            oauth_cookie: AppCookie::oauth(cookie.is_secure()),
            registration_cookie: AppCookie::registration(cookie.is_secure(), verification_ttl),
            device_cookie: AppCookie::device(cookie.is_secure()),
            services,
            cookie,
            limits,
            trusted_proxy_hops,
            background: Background::inline(),
        }
    }

    /// Hands work off with `background` instead of running it before responding.
    #[must_use]
    pub fn with_background(self, background: Background) -> Self {
        Self { background, ..self }
    }
}

// Derived `Clone` would require `A: Clone`, which adapters have no reason to be.
impl<A: Adapters> Clone for AppState<A> {
    fn clone(&self) -> Self {
        Self {
            services: Arc::clone(&self.services),
            cookie: self.cookie,
            mfa_cookie: self.mfa_cookie,
            oauth_cookie: self.oauth_cookie,
            registration_cookie: self.registration_cookie,
            device_cookie: self.device_cookie,
            limits: Arc::clone(&self.limits),
            translator: Arc::clone(&self.translator),
            trusted_proxy_hops: self.trusted_proxy_hops,
            background: self.background.clone(),
        }
    }
}
