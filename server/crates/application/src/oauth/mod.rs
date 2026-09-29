//! Signing up and in with a social account (Google, Apple, GitHub, Microsoft), and linking one to
//! an existing account.
//!
//! [`OAuthService::start`] stores an `OAuthFlow` (state digest, PKCE verifier, nonce, whether it
//! links an account, where to continue) and returns the provider URL plus the `state`, which the
//! route puts in the `oauth` cookie. [`OAuthService::callback`] requires the query `state` to equal
//! the cookie (binding the flow to the browser that started it), consumes the flow once, redeems
//! the code and then either links the account to the signed-in user or signs in through
//! `auth::signin::complete_first_step`, so a second step still applies.
//!
//! Invariants: a provider account never takes over an existing account by email, and only a
//! provider that vouches for the address (`email_verified`) may create an account. Linking needs a
//! verified address and a recent sign-in. The redirect target must be a same-site path.
//!
//! Not a per-entity feature. To add a provider, extend the infrastructure side: the `Provider` list
//! in `infrastructure::oauth` and its profile parsing, plus the configuration; this layer only
//! talks to the `domain::identity::IdentityProviders` port.

pub use service::{CallbackParams, OAuthCallback, OAuthOutcome, OAuthService, OAuthStart};

/// The id of a configured social provider (`google`, `github`). Only `OAuthService` makes one,
/// after checking the configuration, so code past that point cannot act on a provider that is not
/// set up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderId(String);

impl ProviderId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ProviderId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

pub mod dto;

mod service;
