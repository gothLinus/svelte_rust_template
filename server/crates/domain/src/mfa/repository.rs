use std::future::Future;

use time::OffsetDateTime;

use crate::{
    error::StorageError,
    mfa::{MfaChallenge, TotpCredential},
    secret::TokenHash,
    user::UserId,
};

/// Storage for the second step of sign-in. Recovery codes and challenges are stored as
/// [`TokenHash`] digests; the authenticator app's secret is stored sealed
/// ([`Crypto::seal`](crate::security::Crypto::seal)).
#[diagnostic::on_unimplemented(
    message = "`{Self}` does not implement `MfaRepository`",
    note = "every port a service uses must be part of `application::Store` (context.rs: the trait and its blanket impl) and implemented for `PgExecutor<C>` in infrastructure/src/db/repositories/"
)]
pub trait MfaRepository: Send {
    fn find_totp(
        &mut self,
        user: UserId,
    ) -> impl Future<Output = Result<Option<TotpCredential>, StorageError>> + Send;

    fn start_totp_setup(
        &mut self,
        user: UserId,
        sealed_secret: &[u8],
        at: OffsetDateTime,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send;

    fn delete_stale_totp_setups(
        &mut self,
        cutoff: OffsetDateTime,
    ) -> impl Future<Output = Result<u64, StorageError>> + Send;

    fn confirm_totp(
        &mut self,
        user: UserId,
        at: OffsetDateTime,
        step: i64,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send;

    /// Records that a code for `step` was used, if `step` is newer than the last one. Returns
    /// `false` for a replayed code, atomically, so two requests racing with the same code cannot
    /// both succeed.
    fn use_totp_step(
        &mut self,
        user: UserId,
        step: i64,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send;

    fn delete_totp(
        &mut self,
        user: UserId,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send;

    fn replace_recovery_codes(
        &mut self,
        user: UserId,
        code_hashes: &[TokenHash],
    ) -> impl Future<Output = Result<(), StorageError>> + Send;

    fn consume_recovery_code(
        &mut self,
        user: UserId,
        code_hash: &TokenHash,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send;

    fn count_recovery_codes(
        &mut self,
        user: UserId,
    ) -> impl Future<Output = Result<u64, StorageError>> + Send;

    fn create_mfa_challenge(
        &mut self,
        challenge: &MfaChallenge,
    ) -> impl Future<Output = Result<(), StorageError>> + Send;

    fn find_mfa_challenge(
        &mut self,
        token_hash: &TokenHash,
    ) -> impl Future<Output = Result<Option<MfaChallenge>, StorageError>> + Send;

    /// Counts an answer to the challenge *before* it is checked, and returns the challenge (with
    /// the new count) if it was still live at `now`; `None` for a missing, expired or used-up one.
    /// Checking and counting are one atomic step, so concurrent answers cannot get past
    /// [`MAX_MFA_ATTEMPTS`](super::MAX_MFA_ATTEMPTS).
    fn reserve_mfa_attempt(
        &mut self,
        token_hash: &TokenHash,
        now: OffsetDateTime,
    ) -> impl Future<Output = Result<Option<MfaChallenge>, StorageError>> + Send;

    fn delete_mfa_challenge(
        &mut self,
        token_hash: &TokenHash,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send;

    fn delete_user_mfa_challenges(
        &mut self,
        user: UserId,
    ) -> impl Future<Output = Result<u64, StorageError>> + Send;

    fn delete_expired_mfa_challenges(
        &mut self,
        now: OffsetDateTime,
    ) -> impl Future<Output = Result<u64, StorageError>> + Send;
}
