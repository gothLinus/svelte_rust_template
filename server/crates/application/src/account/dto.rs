use domain::user::Username;

use crate::{
    dto::SecretInput, error::ValidationErrors, oauth::dto::IdentityDto, passkeys::dto::PasskeyDto,
    passwordless::dto::TextChannel,
};

#[derive(Debug)]
pub struct UpdateProfileRequest {
    pub username: String,
}

#[derive(Debug)]
pub struct ProfileChange {
    pub username: Username,
}

impl TryFrom<UpdateProfileRequest> for ProfileChange {
    type Error = ValidationErrors;

    fn try_from(request: UpdateProfileRequest) -> Result<Self, Self::Error> {
        let mut errors = ValidationErrors::new();
        match errors.check("username", Username::parse(&request.username)) {
            Some(username) => Ok(Self { username }),
            None => Err(errors),
        }
    }
}

/// The language for mail and texts: one the catalog has, or `None` for the server's default.
#[derive(Debug)]
pub struct SetLocaleRequest {
    pub locale: Option<String>,
}

#[derive(Debug)]
pub struct ChangeEmailRequest {
    pub email: String,
}

#[derive(Debug)]
pub struct AddPhoneRequest {
    pub phone: String,
    pub channel: TextChannel,
}

#[derive(Debug)]
pub struct DeleteAccountRequest {
    /// The account's password, if it has one: confirms the deletion when the session did not sign
    /// in or re-authenticate recently.
    pub password: Option<SecretInput>,
}

#[derive(Debug)]
pub struct SecurityDto {
    pub has_password: bool,
    pub mfa_enabled: bool,
    pub totp_enabled: bool,
    pub recovery_codes_remaining: u32,
    pub passkeys: Vec<PasskeyDto>,
    pub linked_accounts: Vec<IdentityDto>,
}
