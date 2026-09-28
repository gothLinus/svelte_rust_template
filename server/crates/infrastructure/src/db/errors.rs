//! Translation of sqlx errors into [`StorageError`], so callers can react to constraint violations
//! without knowing about sqlx or Postgres error codes.
//!
//! Unique and foreign-key violations keep the constraint's name from the schema (such as
//! `users_email_lower_key`), which services match with [`StorageError::is_unique_violation`]
//! against a constant next to the entity; renaming a constraint in a migration therefore means
//! updating that constant. Every other sqlx error is a [`StorageError::Backend`].

use std::error::Error as StdError;

use domain::error::StorageError;
use sqlx::error::ErrorKind;

/// Maps a sqlx error to a [`StorageError`]. Every repository method ends its query with
/// `.map_err(db_error)`.
///
/// `RowNotFound` stays a backend error: repositories use `fetch_optional` where a row may be
/// missing, so an empty `fetch_one` is a bug to log as a 500, not a 404.
pub(crate) fn db_error(err: sqlx::Error) -> StorageError {
    if let sqlx::Error::Database(db) = &err {
        let constraint = db.constraint().unwrap_or_default().to_owned();
        match db.kind() {
            ErrorKind::UniqueViolation => return StorageError::UniqueViolation { constraint },
            ErrorKind::ForeignKeyViolation => {
                return StorageError::ForeignKeyViolation { constraint };
            }
            _ => {}
        }
    }
    StorageError::backend(err)
}

/// A stored value that no longer passes domain validation, such as a row edited by hand. Used
/// when turning a row into a domain value with the type's `parse`.
pub(crate) fn corrupt(err: impl StdError + Send + Sync + 'static) -> StorageError {
    StorageError::corrupt(err)
}

pub(crate) fn to_i64(value: u32) -> i64 {
    i64::from(value)
}

pub(crate) fn to_u64(value: i64) -> Result<u64, StorageError> {
    u64::try_from(value).map_err(corrupt)
}
