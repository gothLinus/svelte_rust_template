//! `OAUTH_*`: the social sign-in providers. A provider is enabled by setting its
//! `OAUTH_<PROVIDER>_CLIENT_ID`; its other variables are then required. The provider profiles
//! (endpoints, scopes, claims) live in `crate::oauth`.

use domain::secret::Secret;

use super::reader::Reader;
use crate::oauth::{AppleKey, Provider};

#[derive(Debug, Clone, Default)]
pub struct OAuthConfig {
    pub providers: Vec<OAuthProviderConfig>,
}

#[derive(Debug, Clone)]
pub struct OAuthProviderConfig {
    pub provider: Provider,
    pub client_id: String,
    pub secret: ClientSecret,
    pub microsoft_tenant: Option<String>,
}

#[derive(Debug, Clone)]
pub enum ClientSecret {
    Shared(Secret),
    Apple(AppleKey),
}

impl Reader<'_> {
    pub(super) fn oauth(&mut self) -> OAuthConfig {
        let mut providers = Vec::new();
        for provider in Provider::ALL {
            let Some(client_id) = self.raw(provider.client_id_var()) else {
                continue;
            };
            let secret = if provider == Provider::Apple {
                let team_id = self.required("OAUTH_APPLE_TEAM_ID", |raw| Ok(raw.to_owned()));
                let key_id = self.required("OAUTH_APPLE_KEY_ID", |raw| Ok(raw.to_owned()));
                let pem = self.required(provider.client_secret_var(), |raw| Ok(raw.to_owned()));
                match (team_id, key_id, pem) {
                    (Some(team_id), Some(key_id), Some(pem)) => {
                        match AppleKey::new(team_id, key_id, &pem) {
                            Ok(key) => Some(ClientSecret::Apple(key)),
                            Err(err) => {
                                self.problem(provider.client_secret_var(), &err.to_string());
                                None
                            }
                        }
                    }
                    _ => None,
                }
            } else {
                self.required(provider.client_secret_var(), |raw| {
                    Ok(ClientSecret::Shared(Secret::new(raw)))
                })
            };
            let microsoft_tenant = if provider == Provider::Microsoft {
                self.raw("OAUTH_MICROSOFT_TENANT")
            } else {
                None
            };
            if let Some(secret) = secret {
                providers.push(OAuthProviderConfig {
                    provider,
                    client_id,
                    secret,
                    microsoft_tenant,
                });
            }
        }
        OAuthConfig { providers }
    }
}

pub(super) fn is_provider_var(name: &str) -> bool {
    Provider::ALL.iter().any(|provider| {
        provider.client_id_var() == name
            || provider.client_secret_var() == name
            || provider.extra_vars().contains(&name)
    })
}
