use application::{
    ValidationErrors,
    auth::dto::{
        AuthMethodsDto, CancelEmailChangeRequest, ConfirmEmailRequest, ForgotPasswordRequest,
        LinkLifetimesDto, LoginRequest, ReauthMethod, ReauthMethodsDto, ReauthenticateRequest,
        RegisterRequest, ResetPasswordRequest, VerificationPending, VerifyEmailRequest,
    },
    dto::SecretInput,
};
use proto::v1;

use super::{FromMessage, IntoMessage, invalid_enum, passwordless::text_channel_message};

impl FromMessage for RegisterRequest {
    type Message = v1::RegisterRequest;

    fn from_message(message: v1::RegisterRequest) -> Result<Self, ValidationErrors> {
        Ok(Self {
            email: message.email,
            username: message.username,
            password: SecretInput::from(message.password),
        })
    }
}

impl IntoMessage for VerificationPending {
    type Message = v1::VerificationPending;

    fn into_message(self) -> v1::VerificationPending {
        v1::VerificationPending { email: self.email }
    }
}

impl FromMessage for LoginRequest {
    type Message = v1::LoginRequest;

    fn from_message(message: v1::LoginRequest) -> Result<Self, ValidationErrors> {
        Ok(Self {
            identifier: message.identifier,
            password: SecretInput::from(message.password),
        })
    }
}

impl FromMessage for VerifyEmailRequest {
    type Message = v1::VerifyEmailRequest;

    fn from_message(message: v1::VerifyEmailRequest) -> Result<Self, ValidationErrors> {
        Ok(Self {
            token: SecretInput::from(message.token),
        })
    }
}

impl FromMessage for ForgotPasswordRequest {
    type Message = v1::ForgotPasswordRequest;

    fn from_message(message: v1::ForgotPasswordRequest) -> Result<Self, ValidationErrors> {
        Ok(Self {
            email: message.email,
        })
    }
}

impl FromMessage for ResetPasswordRequest {
    type Message = v1::ResetPasswordRequest;

    fn from_message(message: v1::ResetPasswordRequest) -> Result<Self, ValidationErrors> {
        Ok(Self {
            token: SecretInput::from(message.token),
            password: SecretInput::from(message.password),
        })
    }
}

impl FromMessage for ConfirmEmailRequest {
    type Message = v1::ConfirmEmailRequest;

    fn from_message(message: v1::ConfirmEmailRequest) -> Result<Self, ValidationErrors> {
        Ok(Self {
            token: SecretInput::from(message.token),
        })
    }
}

impl FromMessage for CancelEmailChangeRequest {
    type Message = v1::CancelEmailChangeRequest;

    fn from_message(message: v1::CancelEmailChangeRequest) -> Result<Self, ValidationErrors> {
        Ok(Self {
            token: SecretInput::from(message.token),
        })
    }
}

impl IntoMessage for AuthMethodsDto {
    type Message = v1::AuthMethods;

    fn into_message(self) -> v1::AuthMethods {
        v1::AuthMethods {
            app_name: self.app_name,
            providers: self
                .providers
                .into_iter()
                .map(IntoMessage::into_message)
                .collect(),
            text_channels: self
                .text_channels
                .into_iter()
                .map(|channel| text_channel_message(channel).into())
                .collect(),
            lifetimes: Some(self.lifetimes.into_message()),
        }
    }
}

impl IntoMessage for LinkLifetimesDto {
    type Message = v1::LinkLifetimes;

    fn into_message(self) -> v1::LinkLifetimes {
        v1::LinkLifetimes {
            sign_in_minutes: self.sign_in_minutes,
            text_code_minutes: self.text_code_minutes,
            password_reset_minutes: self.password_reset_minutes,
            email_verification_hours: self.email_verification_hours,
            email_change_hours: self.email_change_hours,
        }
    }
}

fn reauth_method(method: ReauthMethod) -> v1::ReauthMethod {
    match method {
        ReauthMethod::Password => v1::ReauthMethod::Password,
        ReauthMethod::Totp => v1::ReauthMethod::Totp,
        ReauthMethod::Passkey => v1::ReauthMethod::Passkey,
        ReauthMethod::EmailCode => v1::ReauthMethod::EmailCode,
    }
}

impl IntoMessage for ReauthMethodsDto {
    type Message = v1::ReauthMethods;

    fn into_message(self) -> v1::ReauthMethods {
        v1::ReauthMethods {
            methods: self
                .methods
                .into_iter()
                .map(|method| reauth_method(method).into())
                .collect(),
        }
    }
}

impl FromMessage for ReauthenticateRequest {
    type Message = v1::ReauthenticateRequest;

    fn from_message(message: v1::ReauthenticateRequest) -> Result<Self, ValidationErrors> {
        let secret = SecretInput::from(message.secret);
        match v1::ReauthMethod::try_from(message.method) {
            Ok(v1::ReauthMethod::Password) => Ok(Self::Password { password: secret }),
            Ok(v1::ReauthMethod::Totp) => Ok(Self::Totp { code: secret }),
            Ok(v1::ReauthMethod::EmailCode) => Ok(Self::EmailCode { code: secret }),
            Ok(v1::ReauthMethod::Passkey | v1::ReauthMethod::Unspecified) | Err(_) => {
                Err(invalid_enum("method", message.method))
            }
        }
    }
}
