//! Request and response bodies of the passkey endpoints.
//!
//! Binary values (challenges, credential ids, WebAuthn responses) are raw bytes, as the browser's
//! `navigator.credentials` produces and takes them. The options mirror the WebAuthn
//! `PublicKeyCredentialCreationOptions` and `PublicKeyCredentialRequestOptions`.

use domain::passkey::Passkey;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct RelyingPartyDto {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct PasskeyUserDto {
    pub id: Vec<u8>,
    pub name: String,
    pub display_name: String,
}

#[derive(Debug, Clone)]
pub struct CredentialParameterDto {
    pub kind: String,
    pub alg: i64,
}

#[derive(Debug, Clone)]
pub struct CredentialDescriptorDto {
    pub kind: String,
    pub id: Vec<u8>,
    pub transports: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AuthenticatorSelectionDto {
    pub resident_key: String,
    pub user_verification: String,
}

#[derive(Debug, Clone)]
pub struct PublicKeyCreationOptionsDto {
    pub rp: RelyingPartyDto,
    pub user: PasskeyUserDto,
    pub challenge: Vec<u8>,
    pub pub_key_cred_params: Vec<CredentialParameterDto>,
    pub timeout: u32,
    pub exclude_credentials: Vec<CredentialDescriptorDto>,
    pub authenticator_selection: AuthenticatorSelectionDto,
    pub attestation: String,
}

#[derive(Debug, Clone)]
pub struct PublicKeyRequestOptionsDto {
    pub challenge: Vec<u8>,
    pub rp_id: String,
    pub timeout: u32,
    pub user_verification: String,
    pub allow_credentials: Vec<CredentialDescriptorDto>,
}

#[derive(Debug, Clone)]
pub struct PasskeyCreationOptionsDto {
    pub challenge_id: Uuid,
    pub public_key: PublicKeyCreationOptionsDto,
}

#[derive(Debug, Clone)]
pub struct PasskeyRequestOptionsDto {
    pub challenge_id: Uuid,
    pub public_key: PublicKeyRequestOptionsDto,
}

#[derive(Debug)]
pub struct RegisterPasskeyRequest {
    pub challenge_id: Uuid,
    pub name: String,
    pub credential_id: Vec<u8>,
    pub client_data_json: Vec<u8>,
    pub authenticator_data: Vec<u8>,
    pub public_key: Vec<u8>,
    pub public_key_algorithm: i64,
    pub transports: Vec<String>,
}

#[derive(Debug)]
pub struct PasskeyAssertionRequest {
    pub challenge_id: Uuid,
    pub credential_id: Vec<u8>,
    pub client_data_json: Vec<u8>,
    pub authenticator_data: Vec<u8>,
    pub signature: Vec<u8>,
    pub user_handle: Option<Vec<u8>>,
}

#[derive(Debug)]
pub struct RenamePasskeyRequest {
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct PasskeyDto {
    pub id: Uuid,
    pub name: String,
    pub created_at: OffsetDateTime,
    pub last_used_at: Option<OffsetDateTime>,
}

impl From<&Passkey> for PasskeyDto {
    fn from(passkey: &Passkey) -> Self {
        Self {
            id: passkey.id.as_uuid(),
            name: passkey.name.to_string(),
            created_at: passkey.created_at,
            last_used_at: passkey.last_used_at,
        }
    }
}

#[derive(Debug)]
pub struct PasskeyRegisteredDto {
    pub passkey: PasskeyDto,
    pub recovery_codes: Option<Vec<String>>,
}
