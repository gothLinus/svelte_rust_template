use std::future::Future;

use time::OffsetDateTime;

use crate::{
    error::StorageError,
    passkey::{
        ChallengeId, ChallengePurpose, NewPasskey, Passkey, PasskeyId, PasskeyName,
        WebAuthnChallenge,
    },
    user::UserId,
};

#[diagnostic::on_unimplemented(
    message = "`{Self}` does not implement `PasskeyRepository`",
    note = "every port a service uses must be part of `application::Store` (context.rs: the trait and its blanket impl) and implemented for `PgExecutor<C>` in infrastructure/src/db/repositories/"
)]
pub trait PasskeyRepository: Send {
    /// Fails with a unique violation on
    /// [`PASSKEY_CREDENTIAL_UNIQUE_CONSTRAINT`](crate::passkey::PASSKEY_CREDENTIAL_UNIQUE_CONSTRAINT)
    /// if the credential is registered already.
    fn create_passkey(
        &mut self,
        passkey: &NewPasskey,
    ) -> impl Future<Output = Result<Passkey, StorageError>> + Send;

    fn list_user_passkeys(
        &mut self,
        user: UserId,
    ) -> impl Future<Output = Result<Vec<Passkey>, StorageError>> + Send;

    fn find_passkey_by_credential(
        &mut self,
        credential_id: &[u8],
    ) -> impl Future<Output = Result<Option<Passkey>, StorageError>> + Send;

    fn record_passkey_use(
        &mut self,
        id: PasskeyId,
        sign_count: u32,
        at: OffsetDateTime,
    ) -> impl Future<Output = Result<(), StorageError>> + Send;

    fn rename_user_passkey(
        &mut self,
        user: UserId,
        id: PasskeyId,
        name: &PasskeyName,
    ) -> impl Future<Output = Result<Option<Passkey>, StorageError>> + Send;

    fn delete_user_passkey(
        &mut self,
        user: UserId,
        id: PasskeyId,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send;

    fn delete_user_passkeys(
        &mut self,
        user: UserId,
    ) -> impl Future<Output = Result<u64, StorageError>> + Send;

    fn count_user_passkeys(
        &mut self,
        user: UserId,
    ) -> impl Future<Output = Result<u64, StorageError>> + Send;

    fn create_webauthn_challenge(
        &mut self,
        challenge: &WebAuthnChallenge,
    ) -> impl Future<Output = Result<(), StorageError>> + Send;

    /// Deletes and returns the challenge if it has this purpose and has not expired, so each one is
    /// answered at most once.
    fn consume_webauthn_challenge(
        &mut self,
        id: ChallengeId,
        purpose: ChallengePurpose,
        now: OffsetDateTime,
    ) -> impl Future<Output = Result<Option<WebAuthnChallenge>, StorageError>> + Send;

    fn delete_expired_webauthn_challenges(
        &mut self,
        now: OffsetDateTime,
    ) -> impl Future<Output = Result<u64, StorageError>> + Send;
}
