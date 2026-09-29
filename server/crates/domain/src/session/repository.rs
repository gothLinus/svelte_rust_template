use std::future::Future;

use time::OffsetDateTime;

use crate::{
    error::StorageError,
    secret::TokenHash,
    session::{AuthenticatedSession, Session, SessionId},
    user::UserId,
};

/// Storage for sessions. Revoking a session deletes it. Only token digests are stored
/// ([`TokenHash`]), never the token in the cookie. A `bool` result says whether a row matched, so
/// `false` means the session no longer exists.
#[diagnostic::on_unimplemented(
    message = "`{Self}` does not implement `SessionRepository`",
    note = "every port a service uses must be part of `application::Store` (context.rs: the trait and its blanket impl) and implemented for `PgExecutor<C>` in infrastructure/src/db/repositories/"
)]
pub trait SessionRepository: Send {
    fn create_session(
        &mut self,
        session: &Session,
    ) -> impl Future<Output = Result<(), StorageError>> + Send;

    /// The session with this token digest, its user and their permissions, expired or not. Checking
    /// expiry is the caller's job, because it depends on the
    /// [`SessionPolicy`](crate::session::SessionPolicy).
    fn find_session_by_token(
        &mut self,
        token_hash: &TokenHash,
    ) -> impl Future<Output = Result<Option<AuthenticatedSession>, StorageError>> + Send;

    fn touch_session(
        &mut self,
        session: &Session,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send;

    /// Persists a new token digest and `last_seen_at` and clears the pending rotation (see
    /// [`Session::rotate`]), but only while the stored digest is still `previous`.
    ///
    /// Returns `false` if a concurrent request rotated the token first; the caller must then not
    /// hand out its new token, which was never stored.
    fn rotate_session(
        &mut self,
        session: &Session,
        previous: &TokenHash,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send;

    fn mark_session_reauthenticated(
        &mut self,
        id: SessionId,
        at: OffsetDateTime,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send;

    /// Marks every session of the user for token rotation on its next request, for when the user's
    /// roles change.
    fn flag_user_sessions_for_rotation(
        &mut self,
        user: UserId,
    ) -> impl Future<Output = Result<u64, StorageError>> + Send;

    fn delete_session(
        &mut self,
        id: SessionId,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send;

    fn delete_session_by_token(
        &mut self,
        token_hash: &TokenHash,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send;

    fn delete_user_session(
        &mut self,
        user: UserId,
        id: SessionId,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send;

    fn delete_user_sessions(
        &mut self,
        user: UserId,
        keep: Option<SessionId>,
    ) -> impl Future<Output = Result<u64, StorageError>> + Send;

    /// The user's sessions, most recently used first. May include expired ones that the cleanup
    /// task has not deleted yet.
    fn list_user_sessions(
        &mut self,
        user: UserId,
    ) -> impl Future<Output = Result<Vec<Session>, StorageError>> + Send;

    fn delete_expired_sessions(
        &mut self,
        now: OffsetDateTime,
        idle_cutoff: OffsetDateTime,
    ) -> impl Future<Output = Result<u64, StorageError>> + Send;
}
