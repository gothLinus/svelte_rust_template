use application::{
    ValidationErrors,
    dto::SecretInput,
    passwordless::dto::{
        EmailCodeRequest, MagicLinkRequest, PhoneCodeRequest, TextChannel, VerifyEmailCodeRequest,
        VerifyPhoneCodeRequest,
    },
};
use proto::v1;

use super::{FromMessage, invalid_enum};

pub(super) fn text_channel_message(channel: TextChannel) -> v1::TextChannel {
    match channel {
        TextChannel::Sms => v1::TextChannel::Sms,
        TextChannel::Whatsapp => v1::TextChannel::Whatsapp,
    }
}

pub(super) fn text_channel(value: i32) -> Result<TextChannel, ValidationErrors> {
    match v1::TextChannel::try_from(value) {
        Ok(v1::TextChannel::Sms) => Ok(TextChannel::Sms),
        Ok(v1::TextChannel::Whatsapp) => Ok(TextChannel::Whatsapp),
        Ok(v1::TextChannel::Unspecified) | Err(_) => Err(invalid_enum("channel", value)),
    }
}

impl FromMessage for EmailCodeRequest {
    type Message = v1::EmailCodeRequest;

    fn from_message(message: v1::EmailCodeRequest) -> Result<Self, ValidationErrors> {
        Ok(Self {
            email: message.email,
        })
    }
}

impl FromMessage for VerifyEmailCodeRequest {
    type Message = v1::VerifyEmailCodeRequest;

    fn from_message(message: v1::VerifyEmailCodeRequest) -> Result<Self, ValidationErrors> {
        Ok(Self {
            email: message.email,
            code: SecretInput::from(message.code),
        })
    }
}

impl FromMessage for MagicLinkRequest {
    type Message = v1::MagicLinkRequest;

    fn from_message(message: v1::MagicLinkRequest) -> Result<Self, ValidationErrors> {
        Ok(Self {
            token: SecretInput::from(message.token),
        })
    }
}

impl FromMessage for PhoneCodeRequest {
    type Message = v1::PhoneCodeRequest;

    fn from_message(message: v1::PhoneCodeRequest) -> Result<Self, ValidationErrors> {
        Ok(Self {
            channel: text_channel(message.channel)?,
            phone: message.phone,
        })
    }
}

impl FromMessage for VerifyPhoneCodeRequest {
    type Message = v1::VerifyPhoneCodeRequest;

    fn from_message(message: v1::VerifyPhoneCodeRequest) -> Result<Self, ValidationErrors> {
        Ok(Self {
            phone: message.phone,
            code: SecretInput::from(message.code),
        })
    }
}
