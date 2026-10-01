use application::{
    ValidationErrors,
    account::dto::{
        AddPhoneRequest, ChangeEmailRequest, DeleteAccountRequest, SecurityDto, SetLocaleRequest,
        UpdateProfileRequest,
    },
    dto::SecretInput,
};
use proto::v1;

use super::{FromMessage, IntoMessage, passwordless::text_channel};

impl FromMessage for UpdateProfileRequest {
    type Message = v1::UpdateProfileRequest;

    fn from_message(message: v1::UpdateProfileRequest) -> Result<Self, ValidationErrors> {
        Ok(Self {
            username: message.username,
        })
    }
}

impl FromMessage for SetLocaleRequest {
    type Message = v1::SetLocaleRequest;

    fn from_message(message: v1::SetLocaleRequest) -> Result<Self, ValidationErrors> {
        Ok(Self {
            locale: message.locale,
        })
    }
}

impl FromMessage for ChangeEmailRequest {
    type Message = v1::ChangeEmailRequest;

    fn from_message(message: v1::ChangeEmailRequest) -> Result<Self, ValidationErrors> {
        Ok(Self {
            email: message.email,
        })
    }
}

impl FromMessage for AddPhoneRequest {
    type Message = v1::AddPhoneRequest;

    fn from_message(message: v1::AddPhoneRequest) -> Result<Self, ValidationErrors> {
        Ok(Self {
            channel: text_channel(message.channel)?,
            phone: message.phone,
        })
    }
}

impl FromMessage for DeleteAccountRequest {
    type Message = v1::DeleteAccountRequest;

    fn from_message(message: v1::DeleteAccountRequest) -> Result<Self, ValidationErrors> {
        Ok(Self {
            password: message.password.map(SecretInput::from),
        })
    }
}

impl IntoMessage for SecurityDto {
    type Message = v1::SecurityOverview;

    fn into_message(self) -> v1::SecurityOverview {
        v1::SecurityOverview {
            has_password: self.has_password,
            mfa_enabled: self.mfa_enabled,
            totp_enabled: self.totp_enabled,
            recovery_codes_remaining: self.recovery_codes_remaining,
            passkeys: self
                .passkeys
                .into_iter()
                .map(IntoMessage::into_message)
                .collect(),
            linked_accounts: self
                .linked_accounts
                .into_iter()
                .map(IntoMessage::into_message)
                .collect(),
        }
    }
}
