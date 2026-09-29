use domain::{
    one_time_code::CODE_TTL,
    secret::Secret,
    user::{Email, NewPassword, Username},
    user_token::TokenPolicy,
};

use crate::{
    dto::SecretInput, error::ValidationErrors, oauth::dto::ProviderDto,
    passwordless::dto::TextChannel,
};

#[derive(Debug)]
pub struct RegisterRequest {
    pub email: String,
    /// A unique handle, shown in place of a name and usable to sign in instead of the email
    /// address.
    pub username: String,
    pub password: SecretInput,
}

#[derive(Debug)]
pub struct Registration {
    pub email: Email,
    pub username: Username,
    pub password: NewPassword,
}

impl TryFrom<RegisterRequest> for Registration {
    type Error = ValidationErrors;

    fn try_from(request: RegisterRequest) -> Result<Self, Self::Error> {
        let mut errors = ValidationErrors::new();
        let email = errors.check("email", Email::parse(&request.email));
        let username = errors.check("username", Username::parse(&request.username));
        let password = errors.check("password", NewPassword::parse(request.password.0));

        match (email, username, password) {
            (Some(email), Some(username), Some(password)) => Ok(Self {
                email,
                username,
                password,
            }),
            _ => Err(errors),
        }
    }
}

/// Returned instead of a session when sign-in requires a verified address.
#[derive(Debug)]
pub struct VerificationPending {
    pub email: String,
}

#[derive(Debug)]
pub struct LoginRequest {
    pub identifier: String,
    pub password: SecretInput,
}

#[derive(Debug)]
pub struct VerifyEmailRequest {
    pub token: SecretInput,
}

#[derive(Debug)]
pub struct ForgotPasswordRequest {
    pub email: String,
}

/// The token from the reset link and the new password, as received. `TryFrom` turns it into a
/// validated [`PasswordReset`].
#[derive(Debug)]
pub struct ResetPasswordRequest {
    pub token: SecretInput,
    pub password: SecretInput,
}

#[derive(Debug)]
pub struct PasswordReset {
    pub token: Secret,
    pub password: NewPassword,
}

impl TryFrom<ResetPasswordRequest> for PasswordReset {
    type Error = ValidationErrors;

    fn try_from(request: ResetPasswordRequest) -> Result<Self, Self::Error> {
        let mut errors = ValidationErrors::new();
        let password = errors.check("password", NewPassword::parse(request.password.0));
        if request.token.0.is_empty() {
            errors.add("token", &domain::error::ValidationError::required());
        }

        match password {
            Some(password) if errors.is_empty() => Ok(Self {
                token: request.token.0,
                password,
            }),
            _ => Err(errors),
        }
    }
}

#[derive(Debug)]
pub struct ConfirmEmailRequest {
    pub token: SecretInput,
}

#[derive(Debug)]
pub struct CancelEmailChangeRequest {
    pub token: SecretInput,
}

#[derive(Debug)]
pub struct AuthMethodsDto {
    pub app_name: String,
    pub providers: Vec<ProviderDto>,
    pub text_channels: Vec<TextChannel>,
    pub lifetimes: LinkLifetimesDto,
}

/// How long links and codes work, in the unit the mails state them in (see `crate::mail`), so the
/// pages that promise a deadline say the configured one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkLifetimesDto {
    pub sign_in_minutes: u32,
    pub text_code_minutes: u32,
    pub password_reset_minutes: u32,
    pub email_verification_hours: u32,
    pub email_change_hours: u32,
}

impl From<TokenPolicy> for LinkLifetimesDto {
    fn from(tokens: TokenPolicy) -> Self {
        Self {
            sign_in_minutes: whole(tokens.magic_link_ttl.whole_minutes()),
            text_code_minutes: whole(CODE_TTL.whole_minutes()),
            password_reset_minutes: whole(tokens.password_reset_ttl.whole_minutes()),
            email_verification_hours: whole(tokens.email_verification_ttl.whole_hours()),
            email_change_hours: whole(tokens.email_change_ttl.whole_hours()),
        }
    }
}

/// A count of whole units; the configuration keeps every lifetime positive and below a week, so
/// this never saturates in practice.
fn whole(units: i64) -> u32 {
    u32::try_from(units.max(0)).unwrap_or(u32::MAX)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReauthMethod {
    Password,
    Totp,
    Passkey,
    EmailCode,
}

#[derive(Debug, Clone)]
pub struct ReauthMethodsDto {
    pub methods: Vec<ReauthMethod>,
}

/// Re-authenticates the session with a password, an authenticator code or an emailed code.
/// Passkeys have their own ceremony endpoints.
#[derive(Debug)]
pub enum ReauthenticateRequest {
    Password { password: SecretInput },
    Totp { code: SecretInput },
    EmailCode { code: SecretInput },
}
