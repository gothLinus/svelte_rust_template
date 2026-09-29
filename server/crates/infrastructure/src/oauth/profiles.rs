//! Redeeming the authorization code and reading the profile, per provider.
//!
//! Fetching and parsing are split: the `*_profile` functions turn what a provider answered into a
//! [`ProviderProfile`], and decide whether it vouches for the address. That decision is what keeps
//! someone from claiming another person's address, so it is tested against recorded answers of
//! each provider.

use domain::{
    identity::{OAuthError, ProviderProfile},
    secret::Secret,
};
use reqwest::{Client, RequestBuilder, header::ACCEPT};
use serde::{Deserialize, de::DeserializeOwned};

use crate::{
    config::{ClientSecret, OAuthProviderConfig},
    oauth::{
        Provider,
        id_token::{self, Expected},
    },
};

#[derive(Debug, Deserialize)]
struct TokenResponse {
    access_token: Option<String>,
    id_token: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

/// Redeems the authorization `code` (with the PKCE verifier) at the provider's token endpoint and
/// returns the user's profile.
///
/// OpenID providers are trusted through the ID token from that response (issuer, audience, expiry
/// and `nonce` are checked, see [`id_token`]); GitHub is asked for its user and verified primary
/// email with the access token.
///
/// # Errors
///
/// `OAuthError::Rejected` when the provider refuses the code (an error in the response or a
/// `4xx`); `OAuthError::Failed` for transport errors, unexpected responses and an ID token that
/// fails a check.
pub(crate) async fn exchange(
    http: &Client,
    config: &OAuthProviderConfig,
    redirect_uri: &str,
    code: &Secret,
    pkce_verifier: &Secret,
    nonce: &str,
) -> Result<ProviderProfile, OAuthError> {
    let provider = config.provider;
    let endpoints = provider.endpoints(config.microsoft_tenant.as_deref());
    let client_secret = match &config.secret {
        ClientSecret::Shared(secret) => secret.expose().to_owned(),
        ClientSecret::Apple(key) => key.client_secret(&config.client_id)?,
    };

    let form = [
        ("grant_type", "authorization_code"),
        ("code", code.expose()),
        ("redirect_uri", redirect_uri),
        ("client_id", &config.client_id),
        ("client_secret", &client_secret),
        ("code_verifier", pkce_verifier.expose()),
    ];
    let tokens: TokenResponse = send(http.post(&endpoints.token).form(&form)).await?;
    if let Some(error) = tokens.error {
        return Err(OAuthError::Rejected(
            tokens.error_description.unwrap_or(error),
        ));
    }

    if provider.is_openid() {
        let token = tokens
            .id_token
            .ok_or_else(|| OAuthError::Rejected("no ID token in the response".to_owned()))?;
        let issuer = issuer_check(provider, config.microsoft_tenant.as_deref());
        let claims = id_token::claims_from_token_endpoint(
            &token,
            &Expected {
                issuer: &*issuer,
                client_id: &config.client_id,
                nonce,
            },
        )?;
        return Ok(openid_profile(provider, claims));
    }

    let access_token = tokens
        .access_token
        .ok_or_else(|| OAuthError::Rejected("no access token in the response".to_owned()))?;
    match provider {
        Provider::Github => github(http, &access_token).await,
        Provider::Google | Provider::Apple | Provider::Microsoft => {
            Err(OAuthError::UnknownProvider)
        }
    }
}

/// The profile from an ID token's claims. `email_verified` is only true when the provider vouches
/// for the address, which is what stops someone from signing in as another person's email.
pub fn openid_profile(provider: Provider, claims: id_token::Claims) -> ProviderProfile {
    ProviderProfile {
        subject: claims.subject,
        email: claims.email,
        // Microsoft does not vouch for the `email` claim: any tenant's admin can set it.
        email_verified: claims.email_verified && provider != Provider::Microsoft,
        name: claims.name,
    }
}

/// Whether `iss` is an issuer of `provider`'s ID tokens, with the configured Microsoft tenant
/// (`OAUTH_MICROSOFT_TENANT`).
pub fn accepts_issuer(provider: Provider, microsoft_tenant: Option<&str>, iss: &str) -> bool {
    issuer_check(provider, microsoft_tenant)(iss)
}

const MICROSOFT_CONSUMERS_TENANT: &str = "9188040d-6c67-4c5b-b112-36a304b66dad";

fn issuer_check(provider: Provider, microsoft_tenant: Option<&str>) -> Box<dyn Fn(&str) -> bool> {
    match provider {
        Provider::Google => {
            Box::new(|iss| iss == "https://accounts.google.com" || iss == "accounts.google.com")
        }
        Provider::Apple => Box::new(|iss| iss == "https://appleid.apple.com"),
        Provider::Microsoft => {
            let pinned = match microsoft_tenant {
                // Multi-tenant apps get the user's tenant in the issuer.
                None | Some("common" | "organizations") => None,
                Some("consumers") => Some(MICROSOFT_CONSUMERS_TENANT.to_owned()),
                // A single tenant: its id (or domain) is the only issuer.
                Some(tenant) => Some(tenant.to_owned()),
            };
            match pinned {
                Some(tenant) => {
                    let issuer = format!("https://login.microsoftonline.com/{tenant}/v2.0");
                    Box::new(move |iss| iss == issuer)
                }
                None => Box::new(|iss| {
                    iss.starts_with("https://login.microsoftonline.com/") && iss.ends_with("/v2.0")
                }),
            }
        }
        Provider::Github => Box::new(|_| false),
    }
}

async fn send<T: DeserializeOwned>(request: RequestBuilder) -> Result<T, OAuthError> {
    let response = request
        .header(ACCEPT, "application/json")
        .send()
        .await
        .map_err(OAuthError::failed)?;
    let status = response.status();
    let body = response.bytes().await.map_err(OAuthError::failed)?;
    if status.is_client_error() {
        let text = String::from_utf8_lossy(&body);
        return Err(OAuthError::Rejected(text.chars().take(200).collect()));
    }
    if !status.is_success() {
        return Err(OAuthError::failed(UnexpectedStatus(status)));
    }
    serde_json::from_slice(&body).map_err(OAuthError::failed)
}

#[derive(Debug, thiserror::Error)]
#[error("the provider answered {0}")]
struct UnexpectedStatus(reqwest::StatusCode);

fn parse<T: DeserializeOwned>(value: serde_json::Value) -> Result<T, OAuthError> {
    serde_json::from_value(value).map_err(OAuthError::failed)
}

async fn github(http: &Client, token: &str) -> Result<ProviderProfile, OAuthError> {
    let user = send(http.get("https://api.github.com/user").bearer_auth(token)).await?;
    let emails = send(
        http.get("https://api.github.com/user/emails")
            .bearer_auth(token),
    )
    .await?;
    github_profile(user, emails)
}

/// GitHub's `/user` and `/user/emails`. Only the primary address, and only if GitHub verified it.
pub fn github_profile(
    user: serde_json::Value,
    emails: serde_json::Value,
) -> Result<ProviderProfile, OAuthError> {
    #[derive(Deserialize)]
    struct User {
        id: u64,
        login: String,
        name: Option<String>,
    }
    #[derive(Deserialize)]
    struct Email {
        #[serde(rename = "email")]
        address: String,
        primary: bool,
        verified: bool,
    }

    let user: User = parse(user)?;
    let emails: Vec<Email> = parse(emails)?;
    let email = emails
        .into_iter()
        .find(|email| email.primary && email.verified);

    Ok(ProviderProfile {
        subject: user.id.to_string(),
        email_verified: email.is_some(),
        email: email.map(|email| email.address),
        name: user.name.or(Some(user.login)),
    })
}
