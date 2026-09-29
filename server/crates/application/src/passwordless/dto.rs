use domain::one_time_code::CodeChannel;

use crate::dto::SecretInput;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextChannel {
    Sms,
    Whatsapp,
}

impl From<TextChannel> for CodeChannel {
    fn from(channel: TextChannel) -> Self {
        match channel {
            TextChannel::Sms => Self::Sms,
            TextChannel::Whatsapp => Self::Whatsapp,
        }
    }
}

impl TextChannel {
    pub fn from_code_channel(channel: CodeChannel) -> Option<Self> {
        match channel {
            CodeChannel::Sms => Some(Self::Sms),
            CodeChannel::Whatsapp => Some(Self::Whatsapp),
            CodeChannel::Email => None,
        }
    }
}

#[derive(Debug)]
pub struct EmailCodeRequest {
    pub email: String,
}

#[derive(Debug)]
pub struct VerifyEmailCodeRequest {
    pub email: String,
    pub code: SecretInput,
}

#[derive(Debug)]
pub struct MagicLinkRequest {
    pub token: SecretInput,
}

#[derive(Debug)]
pub struct PhoneCodeRequest {
    pub phone: String,
    pub channel: TextChannel,
}

#[derive(Debug)]
pub struct VerifyPhoneCodeRequest {
    pub phone: String,
    pub code: SecretInput,
}
