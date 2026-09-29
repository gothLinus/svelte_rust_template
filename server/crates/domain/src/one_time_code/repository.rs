use std::future::Future;

use time::OffsetDateTime;

use crate::{
    error::StorageError,
    one_time_code::{CodePurpose, OneTimeCode},
    user::UserId,
};

/// Storage for one-time codes: at most one per user and [`CodePurpose`], stored as a keyed
/// digest, never the code itself.
#[diagnostic::on_unimplemented(
    message = "`{Self}` does not implement `OneTimeCodeRepository`",
    note = "every port a service uses must be part of `application::Store` (context.rs: the trait and its blanket impl) and implemented for `PgExecutor<C>` in infrastructure/src/db/repositories/"
)]
pub trait OneTimeCodeRepository: Send {
    fn replace_one_time_code(
        &mut self,
        code: &OneTimeCode,
    ) -> impl Future<Output = Result<(), StorageError>> + Send;

    fn find_one_time_code(
        &mut self,
        user: UserId,
        purpose: CodePurpose,
    ) -> impl Future<Output = Result<Option<OneTimeCode>, StorageError>> + Send;

    /// Counts a guess *before* it is checked, and returns the code (with the new count) if it was
    /// still live at `now`; `None` for a missing, expired or used-up code. Checking and counting
    /// are one atomic step, so concurrent guesses cannot get past
    /// [`MAX_CODE_ATTEMPTS`](super::MAX_CODE_ATTEMPTS).
    fn reserve_code_attempt(
        &mut self,
        user: UserId,
        purpose: CodePurpose,
        now: OffsetDateTime,
    ) -> impl Future<Output = Result<Option<OneTimeCode>, StorageError>> + Send;

    fn delete_one_time_code(
        &mut self,
        user: UserId,
        purpose: CodePurpose,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send;

    fn delete_expired_one_time_codes(
        &mut self,
        now: OffsetDateTime,
    ) -> impl Future<Output = Result<u64, StorageError>> + Send;
}
