//! Social sign-in over OAuth 2.0 and OpenID Connect: Google, Apple, GitHub and Microsoft.
//!
//! Every flow is the authorization code flow with PKCE (S256). Providers that speak OpenID Connect
//! hand back an ID token straight from their token endpoint over TLS; its claims (issuer,
//! audience, expiry, nonce) are checked, and per OpenID Connect Core 3.1.3.7 the TLS connection
//! stands in for checking its signature. The others are asked for the profile with the access
//! token.

use std::time::Duration;

use domain::{
    identity::{
        AuthorizationRequest, IdentityProviders, OAuthError, OAuthFuture, ProviderInfo,
        ProviderProfile,
    },
    secret::Secret,
};
use url::Url;

pub use apple::{AppleKey, InvalidAppleKey};
pub use providers::Provider;

use crate::config::{OAuthConfig, OAuthProviderConfig, PublicOrigin};

mod apple;
pub mod id_token;
pub mod profiles;
mod providers;

pub type HttpClientError = reqwest::Error;

pub const HTTP_TIMEOUT: Duration = Duration::from_secs(10);

pub fn http_client() -> Result<reqwest::Client, reqwest::Error> {
    let roots = rustls::RootCertStore {
        roots: webpki_roots::TLS_SERVER_ROOTS.to_vec(),
    };
    #[expect(
        clippy::expect_used,
        reason = "cannot fail: ring supports every default protocol version"
    )]
    let tls = rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .expect("ring supports the default protocol versions")
    .with_root_certificates(roots)
    .with_no_client_auth();

    reqwest::Client::builder()
        .tls_backend_preconfigured(tls)
        .timeout(HTTP_TIMEOUT)
        .user_agent(concat!("svelte-rust-template/", env!("CARGO_PKG_VERSION")))
        .build()
}

/// The `IdentityProviders` adapter: builds each provider's authorization URL and redeems the
/// callback's code for a `ProviderProfile`. Only providers with credentials in `OAuthConfig`
/// exist here; anything else is `OAuthError::UnknownProvider`.
///
/// To add a provider: a `Provider` variant with its endpoints and variables, its profile handling
/// and issuer check in `profiles`, and its config in `config::oauth`.
pub struct OAuthProviders {
    providers: Vec<OAuthProviderConfig>,
    infos: Vec<ProviderInfo>,
    public_url: PublicOrigin,
    http: reqwest::Client,
}

impl OAuthProviders {
    pub fn new(config: &OAuthConfig, public_url: PublicOrigin, http: reqwest::Client) -> Self {
        let infos = config
            .providers
            .iter()
            .map(|provider| ProviderInfo {
                id: provider.provider.id().to_owned(),
                name: provider.provider.name().to_owned(),
            })
            .collect();
        Self {
            providers: config.providers.clone(),
            infos,
            public_url,
            http,
        }
    }

    fn find(&self, id: &str) -> Result<&OAuthProviderConfig, OAuthError> {
        self.providers
            .iter()
            .find(|provider| provider.provider.id() == id)
            .ok_or(OAuthError::UnknownProvider)
    }

    pub fn redirect_uri(&self, provider: Provider) -> String {
        format!(
            "{}/api/v1/auth/oauth/{}/callback",
            self.public_url,
            provider.id()
        )
    }
}

impl IdentityProviders for OAuthProviders {
    fn providers(&self) -> &[ProviderInfo] {
        &self.infos
    }

    fn authorization_url(&self, request: &AuthorizationRequest<'_>) -> Result<String, OAuthError> {
        let config = self.find(request.provider)?;
        let endpoints = config
            .provider
            .endpoints(config.microsoft_tenant.as_deref());
        let mut url = Url::parse(&endpoints.authorize).map_err(OAuthError::failed)?;
        {
            let mut query = url.query_pairs_mut();
            query
                .append_pair("response_type", "code")
                .append_pair("client_id", &config.client_id)
                .append_pair("redirect_uri", &self.redirect_uri(config.provider))
                .append_pair("scope", endpoints.scope)
                .append_pair("state", request.state)
                .append_pair("code_challenge", request.pkce_challenge)
                .append_pair("code_challenge_method", "S256");
            if config.provider.is_openid() {
                query.append_pair("nonce", request.nonce);
            }
            for (key, value) in endpoints.extra_authorize_params {
                query.append_pair(key, value);
            }
        }
        Ok(url.into())
    }

    fn exchange<'a>(
        &'a self,
        provider: &'a str,
        code: &'a Secret,
        pkce_verifier: &'a Secret,
        nonce: &'a str,
    ) -> OAuthFuture<'a, ProviderProfile> {
        Box::pin(async move {
            let config = self.find(provider)?;
            let redirect_uri = self.redirect_uri(config.provider);
            profiles::exchange(
                &self.http,
                config,
                &redirect_uri,
                code,
                pkce_verifier,
                nonce,
            )
            .await
        })
    }
}
