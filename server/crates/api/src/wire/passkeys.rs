use application::{
    ValidationErrors,
    passkeys::dto::{
        CredentialDescriptorDto, PasskeyAssertionRequest, PasskeyCreationOptionsDto, PasskeyDto,
        PasskeyRegisteredDto, PasskeyRequestOptionsDto, RegisterPasskeyRequest,
        RenamePasskeyRequest,
    },
};
use proto::v1;

use super::{FromMessage, IntoMessage, mfa::recovery_codes, timestamp, uuid};

fn descriptors(descriptors: Vec<CredentialDescriptorDto>) -> Vec<v1::CredentialDescriptor> {
    descriptors
        .into_iter()
        .map(|descriptor| v1::CredentialDescriptor {
            id: descriptor.id,
            transports: descriptor.transports,
        })
        .collect()
}

impl IntoMessage for PasskeyCreationOptionsDto {
    type Message = v1::PasskeyCreationOptions;

    fn into_message(self) -> v1::PasskeyCreationOptions {
        let options = self.public_key;
        v1::PasskeyCreationOptions {
            challenge_id: self.challenge_id.to_string(),
            public_key: Some(v1::PublicKeyCreationOptions {
                rp: Some(v1::RelyingParty {
                    id: options.rp.id,
                    name: options.rp.name,
                }),
                user: Some(v1::PasskeyUser {
                    id: options.user.id,
                    name: options.user.name,
                    display_name: options.user.display_name,
                }),
                challenge: options.challenge,
                // COSE identifiers are small; one that did not fit would only not be offered.
                algorithms: options
                    .pub_key_cred_params
                    .iter()
                    .filter_map(|param| i32::try_from(param.alg).ok())
                    .collect(),
                timeout: options.timeout,
                exclude_credentials: descriptors(options.exclude_credentials),
                authenticator_selection: Some(v1::AuthenticatorSelection {
                    resident_key: options.authenticator_selection.resident_key,
                    user_verification: options.authenticator_selection.user_verification,
                }),
                attestation: options.attestation,
            }),
        }
    }
}

impl IntoMessage for PasskeyRequestOptionsDto {
    type Message = v1::PasskeyRequestOptions;

    fn into_message(self) -> v1::PasskeyRequestOptions {
        let options = self.public_key;
        v1::PasskeyRequestOptions {
            challenge_id: self.challenge_id.to_string(),
            public_key: Some(v1::PublicKeyRequestOptions {
                challenge: options.challenge,
                rp_id: options.rp_id,
                timeout: options.timeout,
                user_verification: options.user_verification,
                allow_credentials: descriptors(options.allow_credentials),
            }),
        }
    }
}

impl FromMessage for RegisterPasskeyRequest {
    type Message = v1::RegisterPasskeyRequest;

    fn from_message(message: v1::RegisterPasskeyRequest) -> Result<Self, ValidationErrors> {
        Ok(Self {
            challenge_id: uuid("challengeId", &message.challenge_id)?,
            name: message.name,
            credential_id: message.credential_id,
            client_data_json: message.client_data_json,
            authenticator_data: message.authenticator_data,
            public_key: message.public_key,
            public_key_algorithm: i64::from(message.public_key_algorithm),
            transports: message.transports,
        })
    }
}

impl FromMessage for PasskeyAssertionRequest {
    type Message = v1::PasskeyAssertionRequest;

    fn from_message(message: v1::PasskeyAssertionRequest) -> Result<Self, ValidationErrors> {
        Ok(Self {
            challenge_id: uuid("challengeId", &message.challenge_id)?,
            credential_id: message.credential_id,
            client_data_json: message.client_data_json,
            authenticator_data: message.authenticator_data,
            signature: message.signature,
            user_handle: message.user_handle,
        })
    }
}

impl FromMessage for RenamePasskeyRequest {
    type Message = v1::RenamePasskeyRequest;

    fn from_message(message: v1::RenamePasskeyRequest) -> Result<Self, ValidationErrors> {
        Ok(Self { name: message.name })
    }
}

impl IntoMessage for PasskeyDto {
    type Message = v1::Passkey;

    fn into_message(self) -> v1::Passkey {
        v1::Passkey {
            id: self.id.to_string(),
            name: self.name,
            created_at: Some(timestamp(self.created_at)),
            last_used_at: self.last_used_at.map(timestamp),
        }
    }
}

impl IntoMessage for Vec<PasskeyDto> {
    type Message = v1::PasskeyList;

    fn into_message(self) -> v1::PasskeyList {
        v1::PasskeyList {
            passkeys: self.into_iter().map(IntoMessage::into_message).collect(),
        }
    }
}

impl IntoMessage for PasskeyRegisteredDto {
    type Message = v1::PasskeyRegistered;

    fn into_message(self) -> v1::PasskeyRegistered {
        v1::PasskeyRegistered {
            passkey: Some(self.passkey.into_message()),
            recovery_codes: recovery_codes(self.recovery_codes),
        }
    }
}
