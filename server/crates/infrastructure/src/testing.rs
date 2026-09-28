//! Deterministic adapters for tests in other crates. Enabled by the `test-support` feature, which
//! only dev-dependencies turn on.
//!
//! Each fake implements a `domain` port with no network or wall clock: [`ManualClock`] (`Clock`),
//! [`RecordingMailer`] (`Mailer`), [`RecordingTexts`] (`TextSender`), [`FakeIdentityProviders`]
//! (`IdentityProviders`) and [`MemoryObjectStore`] (`ObjectStore`). The recorders are cheap to
//! clone and share their state, so a test keeps one handle to assert on and hands a clone to the
//! app. To fake another port for a test, add its fake here in the same shape rather than in each
//! test crate.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use domain::{
    clock::{Clock, truncate_to_micros},
    identity::{
        AuthorizationRequest, IdentityProviders, OAuthError, OAuthFuture, ProviderInfo,
        ProviderProfile,
    },
    mail::{Mail, MailFuture, Mailer},
    one_time_code::CodeChannel,
    secret::Secret,
    text::{TextFuture, TextMessage, TextSender},
};
use time::{Duration, OffsetDateTime};

#[derive(Debug, Clone)]
pub struct ManualClock(Arc<Mutex<OffsetDateTime>>);

impl ManualClock {
    pub fn new(start: OffsetDateTime) -> Self {
        Self(Arc::new(Mutex::new(truncate_to_micros(start))))
    }

    /// Starts at the current wall-clock time, so rows written with `now()` defaults and rows
    /// written with this clock sort sensibly together.
    pub fn starting_now() -> Self {
        Self::new(OffsetDateTime::now_utc())
    }

    pub fn advance(&self, by: Duration) {
        let mut now = lock(&self.0);
        *now += by;
    }

    pub fn set(&self, to: OffsetDateTime) {
        *lock(&self.0) = truncate_to_micros(to);
    }
}

impl Clock for ManualClock {
    fn now(&self) -> OffsetDateTime {
        *lock(&self.0)
    }
}

#[derive(Debug, Clone, Default)]
pub struct RecordingMailer(Arc<Mutex<Vec<Mail>>>);

impl RecordingMailer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn sent(&self) -> Vec<Mail> {
        lock(&self.0).clone()
    }

    pub fn take(&self) -> Vec<Mail> {
        std::mem::take(&mut *lock(&self.0))
    }

    pub fn last_to(&self, to: &str) -> Option<Mail> {
        lock(&self.0)
            .iter()
            .rev()
            .find(|mail| mail.to.as_str() == to)
            .cloned()
    }
}

impl Mailer for RecordingMailer {
    fn send(&self, mail: Mail) -> MailFuture<'_> {
        lock(&self.0).push(mail);
        Box::pin(async { Ok(()) })
    }
}

/// Keeps every text message instead of sending it. Offers SMS and WhatsApp.
#[derive(Debug, Clone, Default)]
pub struct RecordingTexts(Arc<Mutex<Vec<TextMessage>>>);

impl RecordingTexts {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn sent(&self) -> Vec<TextMessage> {
        lock(&self.0).clone()
    }

    pub fn last_code_to(&self, to: &str) -> Option<String> {
        let messages = lock(&self.0);
        let message = messages
            .iter()
            .rev()
            .find(|message| message.to.as_str() == to)?;
        code_in(&message.body)
    }
}

fn code_in(text: &str) -> Option<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    let digits =
        |word: &str, len: usize| word.len() == len && word.chars().all(|c| c.is_ascii_digit());
    words.iter().enumerate().find_map(|(at, word)| {
        if digits(word, 6) {
            Some((*word).to_owned())
        } else if digits(word, 3) && words.get(at + 1).is_some_and(|next| digits(next, 3)) {
            Some(format!("{word}{}", words[at + 1]))
        } else {
            None
        }
    })
}

impl TextSender for RecordingTexts {
    fn channels(&self) -> &[CodeChannel] {
        &[CodeChannel::Sms, CodeChannel::Whatsapp]
    }

    fn send(&self, message: TextMessage) -> TextFuture<'_> {
        lock(&self.0).push(message);
        Box::pin(async { Ok(()) })
    }
}

/// A single provider, `test`, whose accounts are scripted: the authorization code is the key of a
/// profile registered with [`FakeIdentityProviders::add_account`]. Any other code is rejected like
/// an expired one (`OAuthError::Rejected`).
#[derive(Debug, Clone)]
pub struct FakeIdentityProviders {
    providers: Vec<ProviderInfo>,
    accounts: Arc<Mutex<Vec<(String, ProviderProfile)>>>,
    exchanges: Arc<Mutex<Vec<(String, String, String)>>>,
}

impl Default for FakeIdentityProviders {
    fn default() -> Self {
        Self {
            providers: vec![ProviderInfo {
                id: "test".to_owned(),
                name: "Test Provider".to_owned(),
            }],
            accounts: Arc::default(),
            exchanges: Arc::default(),
        }
    }
}

impl FakeIdentityProviders {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_account(&self, code: &str, profile: ProviderProfile) {
        lock(&self.accounts).push((code.to_owned(), profile));
    }

    /// Every authorization code redeemed so far, as (code, PKCE verifier, nonce): what the flow
    /// under test passed to the provider.
    pub fn exchanges(&self) -> Vec<(String, String, String)> {
        lock(&self.exchanges).clone()
    }
}

impl IdentityProviders for FakeIdentityProviders {
    fn providers(&self) -> &[ProviderInfo] {
        &self.providers
    }

    fn authorization_url(&self, request: &AuthorizationRequest<'_>) -> Result<String, OAuthError> {
        if request.provider != "test" {
            return Err(OAuthError::UnknownProvider);
        }
        Ok(format!(
            "https://provider.test/authorize?state={}&nonce={}&code_challenge={}",
            request.state, request.nonce, request.pkce_challenge
        ))
    }

    fn exchange<'a>(
        &'a self,
        provider: &'a str,
        code: &'a Secret,
        pkce_verifier: &'a Secret,
        nonce: &'a str,
    ) -> OAuthFuture<'a, ProviderProfile> {
        Box::pin(async move {
            if provider != "test" {
                return Err(OAuthError::UnknownProvider);
            }
            lock(&self.exchanges).push((
                code.expose().to_owned(),
                pkce_verifier.expose().to_owned(),
                nonce.to_owned(),
            ));
            lock(&self.accounts)
                .iter()
                .find(|(known, _)| known == code.expose())
                .map(|(_, profile)| profile.clone())
                .ok_or_else(|| OAuthError::Rejected("unknown code".to_owned()))
        })
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The ID token parser of [`crate::oauth::id_token`], for its tests. Production reaches it only
/// through the token exchange, which is the one place its unchecked signature is safe.
pub fn claims_from_token_endpoint(
    token: &str,
    expected: &crate::oauth::id_token::Expected<'_>,
) -> Result<crate::oauth::id_token::Claims, OAuthError> {
    crate::oauth::id_token::claims_from_token_endpoint(token, expected)
}
