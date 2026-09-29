use application::oauth::dto::{IdentityDto, ProviderDto};
use proto::v1;

use super::{IntoMessage, timestamp};

impl IntoMessage for ProviderDto {
    type Message = v1::OAuthProvider;

    fn into_message(self) -> v1::OAuthProvider {
        v1::OAuthProvider {
            id: self.id,
            name: self.name,
        }
    }
}

impl IntoMessage for IdentityDto {
    type Message = v1::LinkedAccount;

    fn into_message(self) -> v1::LinkedAccount {
        v1::LinkedAccount {
            provider: self.provider,
            provider_name: self.provider_name,
            email: self.email,
            created_at: Some(timestamp(self.created_at)),
            last_used_at: Some(timestamp(self.last_used_at)),
        }
    }
}

impl IntoMessage for Vec<IdentityDto> {
    type Message = v1::LinkedAccountList;

    fn into_message(self) -> v1::LinkedAccountList {
        v1::LinkedAccountList {
            accounts: self.into_iter().map(IntoMessage::into_message).collect(),
        }
    }
}
