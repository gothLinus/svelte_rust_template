//! Accounts at social sign-in providers (Google, Apple, GitHub, ...) linked to users, the OAuth
//! 2.0 flows that link them, and the port that talks to the providers.
//!
//! `application::oauth` runs the flow: it stores an [`OAuthFlow`] under the digest of its
//! `state`, sends the browser to the provider ([`IdentityProviders`]), and on the callback
//! consumes the flow once and links or signs in the [`ExternalIdentity`]. Storage is
//! [`IdentityRepository`].

use time::{Duration, OffsetDateTime};

use crate::{
    id::Id,
    secret::{Secret, TokenHash},
    user::UserId,
};

pub use provider::{
    AuthorizationRequest, IdentityProviders, OAuthError, OAuthFuture, ProviderInfo, ProviderProfile,
};
pub use repository::IdentityRepository;

mod provider;
mod repository;

pub type IdentityId = Id<ExternalIdentity>;

pub const IDENTITY_SUBJECT_UNIQUE_CONSTRAINT: &str = "external_identities_provider_subject_key";
pub const IDENTITY_PROVIDER_UNIQUE_CONSTRAINT: &str = "external_identities_user_id_provider_key";

pub const OAUTH_FLOW_TTL: Duration = Duration::minutes(10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalIdentity {
    pub id: IdentityId,
    pub user_id: UserId,
    pub provider: String,
    pub subject: String,
    pub email: Option<String>,
    pub created_at: OffsetDateTime,
    pub last_used_at: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct NewIdentity {
    pub id: IdentityId,
    pub user_id: UserId,
    pub provider: String,
    pub subject: String,
    pub email: Option<String>,
}

/// A sign-in at a provider in progress, stored under the digest of its `state` parameter.
///
/// The PKCE verifier and nonce stay on the server; the browser only carries `state`, in the URL
/// and in a cookie that binds the flow to the browser that started it.
#[derive(Debug, Clone)]
pub struct OAuthFlow {
    pub state_hash: TokenHash,
    pub provider: String,
    pub pkce_verifier: Secret,
    pub nonce: String,
    /// Set when a signed-in user links an account instead of signing in with it.
    pub link_user: Option<UserId>,
    pub redirect_to: String,
    pub expires_at: OffsetDateTime,
}
