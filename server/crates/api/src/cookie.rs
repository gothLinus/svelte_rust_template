use axum::http::{HeaderMap, header::SET_COOKIE};
use axum_extra::extract::{
    CookieJar,
    cookie::{Cookie, SameSite},
};
use domain::secret::Secret;
use time::Duration;

/// A cookie carrying a secret, with the same attributes whatever it is for:
///
/// - `HttpOnly`: JavaScript cannot read it, so an XSS bug cannot steal it.
/// - `SameSite=Lax`: browsers leave it off cross-site subrequests and form posts; it is still sent
///   on top-level navigation, so links into the app keep you signed in.
/// - `Secure` + the `__Host-` prefix (when `secure`): sent over HTTPS only, and browsers refuse a
///   `__Host-` cookie that sets a `Domain`, so no subdomain can plant or overwrite it. The plain
///   name is used for development over `http://`.
/// - `Max-Age` = how long what it carries is valid; the server checks that again.
#[derive(Debug, Clone, Copy)]
pub struct AppCookie {
    name: &'static str,
    secure_name: &'static str,
    secure: bool,
    max_age: Duration,
}

impl AppCookie {
    /// The session token. `max_age` is the session's absolute lifetime; the server enforces the
    /// idle timeout.
    pub fn session(secure: bool, max_age: Duration) -> Self {
        Self {
            name: "session",
            secure_name: "__Host-session",
            secure,
            max_age,
        }
    }

    pub fn mfa(secure: bool) -> Self {
        Self {
            name: "mfa",
            secure_name: "__Host-mfa",
            secure,
            max_age: domain::mfa::MFA_CHALLENGE_TTL,
        }
    }

    pub fn oauth(secure: bool) -> Self {
        Self {
            name: "oauth",
            secure_name: "__Host-oauth",
            secure,
            max_age: domain::identity::OAUTH_FLOW_TTL,
        }
    }

    /// Marks the browser that registered an account until its verification link expires
    /// (`max_age`), so opening the link there keeps the password chosen there.
    pub fn registration(secure: bool, max_age: Duration) -> Self {
        Self {
            name: "registration",
            secure_name: "__Host-registration",
            secure,
            max_age,
        }
    }

    /// Marks a browser the owner signed in on, so strangers' failed sign-ins cannot lock them out
    /// of it.
    pub fn device(secure: bool) -> Self {
        Self {
            name: "device",
            secure_name: "__Host-device",
            secure,
            max_age: application::auth::KNOWN_DEVICE_TTL,
        }
    }

    pub fn is_secure(self) -> bool {
        self.secure
    }

    pub fn name(self) -> &'static str {
        if self.secure {
            self.secure_name
        } else {
            self.name
        }
    }

    pub fn token(self, jar: &CookieJar) -> Option<Secret> {
        jar.get(self.name())
            .map(|cookie| Secret::new(cookie.value()))
            .filter(|token| !token.is_empty())
    }

    pub fn set(self, jar: CookieJar, token: &Secret) -> CookieJar {
        let mut cookie = self.base(token.expose().to_owned());
        cookie.set_max_age(self.max_age);
        jar.add(cookie)
    }

    pub fn clear(self, jar: CookieJar) -> CookieJar {
        jar.remove(self.base(String::new()))
    }

    pub fn is_set_in(self, headers: &HeaderMap) -> bool {
        let prefix = format!("{}=", self.name());
        headers
            .get_all(SET_COOKIE)
            .iter()
            .any(|value| value.as_bytes().starts_with(prefix.as_bytes()))
    }

    fn base(self, value: String) -> Cookie<'static> {
        Cookie::build((self.name(), value))
            .path("/")
            .http_only(true)
            .secure(self.secure)
            .same_site(SameSite::Lax)
            .build()
    }
}
