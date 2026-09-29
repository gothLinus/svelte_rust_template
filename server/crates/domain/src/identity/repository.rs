use std::future::Future;

use time::OffsetDateTime;

use crate::{
    error::StorageError,
    identity::{ExternalIdentity, IdentityId, NewIdentity, OAuthFlow},
    secret::TokenHash,
    user::UserId,
};

#[diagnostic::on_unimplemented(
    message = "`{Self}` does not implement `IdentityRepository`",
    note = "every port a service uses must be part of `application::Store` (context.rs: the trait and its blanket impl) and implemented for `PgExecutor<C>` in infrastructure/src/db/repositories/"
)]
pub trait IdentityRepository: Send {
    /// Fails with a unique violation on
    /// [`IDENTITY_SUBJECT_UNIQUE_CONSTRAINT`](crate::identity::IDENTITY_SUBJECT_UNIQUE_CONSTRAINT)
    /// or
    /// [`IDENTITY_PROVIDER_UNIQUE_CONSTRAINT`](crate::identity::IDENTITY_PROVIDER_UNIQUE_CONSTRAINT).
    fn create_identity(
        &mut self,
        identity: &NewIdentity,
    ) -> impl Future<Output = Result<ExternalIdentity, StorageError>> + Send;

    fn find_identity(
        &mut self,
        provider: &str,
        subject: &str,
    ) -> impl Future<Output = Result<Option<ExternalIdentity>, StorageError>> + Send;

    fn list_user_identities(
        &mut self,
        user: UserId,
    ) -> impl Future<Output = Result<Vec<ExternalIdentity>, StorageError>> + Send;

    fn touch_identity(
        &mut self,
        id: IdentityId,
        at: OffsetDateTime,
    ) -> impl Future<Output = Result<(), StorageError>> + Send;

    fn delete_user_identity(
        &mut self,
        user: UserId,
        provider: &str,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send;

    fn delete_user_identities(
        &mut self,
        user: UserId,
    ) -> impl Future<Output = Result<u64, StorageError>> + Send;

    fn create_oauth_flow(
        &mut self,
        flow: &OAuthFlow,
    ) -> impl Future<Output = Result<(), StorageError>> + Send;

    /// Deletes and returns the flow if it exists and has not expired, so a callback URL works once.
    fn consume_oauth_flow(
        &mut self,
        state_hash: &TokenHash,
        now: OffsetDateTime,
    ) -> impl Future<Output = Result<Option<OAuthFlow>, StorageError>> + Send;

    fn delete_expired_oauth_flows(
        &mut self,
        now: OffsetDateTime,
    ) -> impl Future<Output = Result<u64, StorageError>> + Send;
}
