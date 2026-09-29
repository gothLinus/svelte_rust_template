use crate::dto::SecretInput;

#[derive(Debug)]
pub struct CodeRequest {
    pub code: SecretInput,
}

#[derive(Debug)]
pub struct TotpSetupDto {
    pub secret: String,
    pub uri: String,
}

#[derive(Debug)]
pub struct RecoveryCodesDto {
    pub codes: Vec<String>,
}

/// The result of turning on a second step. `recoveryCodes` is set when two-step sign-in was off
/// before, so the user gets codes for the first time.
#[derive(Debug)]
pub struct SecondFactorAddedDto {
    pub recovery_codes: Option<Vec<String>>,
}
