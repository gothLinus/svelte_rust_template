//! The port to OAuth 2.0 / OpenID Connect providers.
//!
//! Used through `dyn` like the mailer: which providers exist is configuration, and the calls are
//! network round trips where a boxed future costs nothing that matters.

use std::{error::Error as StdError, future::Future, pin::Pin};

use thiserror::Error;

use crate::secret::Secret;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderInfo {
    /// Stable, used in URLs and stored with linked accounts: `google`, `github`.
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Copy)]
pub struct AuthorizationRequest<'a> {
    pub provider: &'a str,
    pub state: &'a str,
    pub nonce: &'a str,
    pub pkce_challenge: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderProfile {
    pub subject: String,
    pub email: Option<String>,
    pub email_verified: bool,
    pub name: Option<String>,
}

#[derive(Debug, Error)]
pub enum OAuthError {
    #[error("unknown sign-in provider")]
    UnknownProvider,
    /// The provider rejected the request, such as for an expired or reused code.
    #[error("the provider rejected the sign-in: {0}")]
    Rejected(String),
    #[error("talking to the sign-in provider failed")]
    Failed(#[source] Box<dyn StdError + Send + Sync>),
}

impl OAuthError {
    pub fn failed(err: impl StdError + Send + Sync + 'static) -> Self {
        Self::Failed(Box::new(err))
    }
}

pub type OAuthFuture<'a, T> = Pin<Box<dyn Future<Output = Result<T, OAuthError>> + Send + 'a>>;

pub trait IdentityProviders: Send + Sync + 'static {
    fn providers(&self) -> &[ProviderInfo];

    fn authorization_url(&self, request: &AuthorizationRequest<'_>) -> Result<String, OAuthError>;

    fn exchange<'a>(
        &'a self,
        provider: &'a str,
        code: &'a Secret,
        pkce_verifier: &'a Secret,
        nonce: &'a str,
    ) -> OAuthFuture<'a, ProviderProfile>;
}
