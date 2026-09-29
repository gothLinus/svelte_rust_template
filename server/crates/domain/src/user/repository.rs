use std::future::Future;

use time::OffsetDateTime;

use crate::{
    error::StorageError,
    pagination::{Page, PageRequest},
    user::{Email, NewUser, PasswordHash, PhoneNumber, User, UserFilter, UserId, Username},
};

/// Storage for accounts. Users are not a generic [`Repository`](crate::repository::Repository)
/// resource: creating one is registration (password hashing, a default role, a session), not a
/// plain insert, and each change has its own rules. Method names carry `user` because one
/// connection type implements every repository trait.
///
/// Emails are matched case-insensitively. A method that changes one user returns `None`, or
/// `false` where it has no row to return, if no such user exists. Unique violations are reported
/// as [`StorageError::UniqueViolation`](crate::error::StorageError) carrying the constraint name,
/// which is how a race between two registrations is told apart from other failures.
#[diagnostic::on_unimplemented(
    message = "`{Self}` does not implement `UserRepository`",
    note = "every port a service uses must be part of `application::Store` (context.rs: the trait and its blanket impl) and implemented for `PgExecutor<C>` in infrastructure/src/db/repositories/"
)]
pub trait UserRepository: Send {
    /// Fails with a unique violation on
    /// [`EMAIL_UNIQUE_CONSTRAINT`](crate::user::EMAIL_UNIQUE_CONSTRAINT) or
    /// [`USERNAME_UNIQUE_CONSTRAINT`](crate::user::USERNAME_UNIQUE_CONSTRAINT) if the email or
    /// username is taken, so racing registrations cannot both succeed.
    fn create_user(
        &mut self,
        user: &NewUser,
    ) -> impl Future<Output = Result<User, StorageError>> + Send;

    fn find_user(
        &mut self,
        id: UserId,
    ) -> impl Future<Output = Result<Option<User>, StorageError>> + Send;

    /// Like [`UserRepository::find_user`], but locks the row until the transaction ends, so a
    /// read-check-write sequence cannot interleave with another one.
    fn find_user_for_update(
        &mut self,
        id: UserId,
    ) -> impl Future<Output = Result<Option<User>, StorageError>> + Send;

    fn find_user_by_email(
        &mut self,
        email: &Email,
    ) -> impl Future<Output = Result<Option<User>, StorageError>> + Send;

    fn find_user_by_username(
        &mut self,
        username: &Username,
    ) -> impl Future<Output = Result<Option<User>, StorageError>> + Send;

    fn find_user_by_phone(
        &mut self,
        phone: &PhoneNumber,
    ) -> impl Future<Output = Result<Option<User>, StorageError>> + Send;

    fn list_users(
        &mut self,
        filter: &UserFilter,
        page: PageRequest,
    ) -> impl Future<Output = Result<Page<User>, StorageError>> + Send;

    /// Changes the username. Fails with a unique violation on
    /// [`USERNAME_UNIQUE_CONSTRAINT`](crate::user::USERNAME_UNIQUE_CONSTRAINT) if it is taken.
    fn set_user_username(
        &mut self,
        id: UserId,
        username: &Username,
    ) -> impl Future<Output = Result<Option<User>, StorageError>> + Send;

    /// Replaces the email address, which was verified at `verified_at`. Fails with a unique
    /// violation on [`EMAIL_UNIQUE_CONSTRAINT`](crate::user::EMAIL_UNIQUE_CONSTRAINT).
    fn change_user_email(
        &mut self,
        id: UserId,
        email: &Email,
        verified_at: OffsetDateTime,
    ) -> impl Future<Output = Result<Option<User>, StorageError>> + Send;

    /// Sets the phone number (verified at `verified_at`), or removes it with `None`. Fails with a
    /// unique violation on [`PHONE_UNIQUE_CONSTRAINT`](crate::user::PHONE_UNIQUE_CONSTRAINT).
    fn set_user_phone(
        &mut self,
        id: UserId,
        phone: Option<(&PhoneNumber, OffsetDateTime)>,
    ) -> impl Future<Output = Result<Option<User>, StorageError>> + Send;

    fn set_user_password(
        &mut self,
        id: UserId,
        hash: Option<&PasswordHash>,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send;

    fn mark_user_email_verified(
        &mut self,
        id: UserId,
        at: OffsetDateTime,
    ) -> impl Future<Output = Result<Option<User>, StorageError>> + Send;

    fn set_user_disabled(
        &mut self,
        id: UserId,
        at: Option<OffsetDateTime>,
    ) -> impl Future<Output = Result<Option<User>, StorageError>> + Send;

    fn delete_user(
        &mut self,
        id: UserId,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send;

    /// Deletes accounts created before `cutoff` whose address was never verified and that hold no
    /// role beyond the default one, and returns how many. Retention: nobody proved such an account
    /// is theirs, so it is personal data without a purpose.
    fn delete_unverified_users(
        &mut self,
        cutoff: OffsetDateTime,
    ) -> impl Future<Output = Result<u64, StorageError>> + Send;
}
