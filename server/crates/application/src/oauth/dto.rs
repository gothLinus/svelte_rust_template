use domain::identity::ExternalIdentity;
use serde::Deserialize;
use time::OffsetDateTime;

#[derive(Debug, Clone)]
pub struct ProviderDto {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct IdentityDto {
    pub provider: String,
    pub provider_name: String,
    pub email: Option<String>,
    pub created_at: OffsetDateTime,
    pub last_used_at: OffsetDateTime,
}

impl IdentityDto {
    pub fn new(identity: &ExternalIdentity, provider_name: &str) -> Self {
        Self {
            provider: identity.provider.clone(),
            provider_name: provider_name.to_owned(),
            email: identity.email.clone(),
            created_at: identity.created_at,
            last_used_at: identity.last_used_at,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OAuthStartQuery {
    pub redirect_to: Option<String>,
}
