use std::future::Future;

use time::OffsetDateTime;

use crate::{
    error::StorageError,
    secret::TokenHash,
    user::UserId,
    user_token::{ConsumedToken, TokenPurpose, UserToken},
};

#[diagnostic::on_unimplemented(
    message = "`{Self}` does not implement `UserTokenRepository`",
    note = "every port a service uses must be part of `application::Store` (context.rs: the trait and its blanket impl) and implemented for `PgExecutor<C>` in infrastructure/src/db/repositories/"
)]
pub trait UserTokenRepository: Send {
    fn replace_user_token(
        &mut self,
        token: &UserToken,
    ) -> impl Future<Output = Result<(), StorageError>> + Send;

    /// Deletes the token if it exists, has this purpose and has not expired, and returns what it
    /// was issued for. A single statement, so two requests racing with the same link cannot both
    /// succeed. Deletes the token if it exists, has this purpose and has not expired, and returns
    /// what it was issued for. A single statement, so two requests racing with the same link cannot
    /// both succeed.
    fn consume_user_token(
        &mut self,
        token_hash: &TokenHash,
        purpose: TokenPurpose,
        now: OffsetDateTime,
    ) -> impl Future<Output = Result<Option<ConsumedToken>, StorageError>> + Send;

    fn user_token_exists(
        &mut self,
        token_hash: &TokenHash,
        purpose: TokenPurpose,
        now: OffsetDateTime,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send;

    fn delete_user_tokens(
        &mut self,
        user: UserId,
        purpose: TokenPurpose,
    ) -> impl Future<Output = Result<u64, StorageError>> + Send;

    fn delete_expired_user_tokens(
        &mut self,
        now: OffsetDateTime,
    ) -> impl Future<Output = Result<u64, StorageError>> + Send;
}
