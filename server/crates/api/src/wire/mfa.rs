use application::{
    ValidationErrors,
    dto::SecretInput,
    mfa::dto::{CodeRequest, RecoveryCodesDto, SecondFactorAddedDto, TotpSetupDto},
};
use proto::v1;

use super::{FromMessage, IntoMessage};

impl FromMessage for CodeRequest {
    type Message = v1::CodeRequest;

    fn from_message(message: v1::CodeRequest) -> Result<Self, ValidationErrors> {
        Ok(Self {
            code: SecretInput::from(message.code),
        })
    }
}

impl IntoMessage for TotpSetupDto {
    type Message = v1::TotpSetup;

    fn into_message(self) -> v1::TotpSetup {
        v1::TotpSetup {
            secret: self.secret,
            uri: self.uri,
        }
    }
}

impl IntoMessage for RecoveryCodesDto {
    type Message = v1::RecoveryCodes;

    fn into_message(self) -> v1::RecoveryCodes {
        v1::RecoveryCodes { codes: self.codes }
    }
}

pub(super) fn recovery_codes(codes: Option<Vec<String>>) -> Option<v1::RecoveryCodes> {
    codes.map(|codes| v1::RecoveryCodes { codes })
}

impl IntoMessage for SecondFactorAddedDto {
    type Message = v1::SecondFactorAdded;

    fn into_message(self) -> v1::SecondFactorAdded {
        v1::SecondFactorAdded {
            recovery_codes: recovery_codes(self.recovery_codes),
        }
    }
}
