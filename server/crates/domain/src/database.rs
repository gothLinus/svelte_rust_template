//! The unit-of-work port: how services get a connection or a transaction without naming a
//! driver.

use std::future::Future;

pub use crate::error::StorageError;

/// A storage backend that hands out connections and transactions.
///
/// Repository traits are implemented on [`Database::Connection`] and
/// [`Database::Transaction`], so the same repository code runs inside or outside a
/// transaction, and a service generic over `D: Database` never sees a driver type:
///
/// ```no_run
/// use domain::{
///     database::{Database, StorageError, Transaction},
///     rbac::{RbacRepository, RoleName},
///     user::{NewUser, User, UserRepository},
/// };
///
/// async fn register<D>(db: &D, new_user: &NewUser) -> Result<User, StorageError>
/// where
///     D: Database<Transaction: UserRepository + RbacRepository>,
/// {
///     let mut tx = db.transaction().await?;
///     let user = tx.create_user(new_user).await?;
///     tx.grant_role(user.id(), &RoleName::USER).await?;
///     tx.commit().await?; // dropping `tx` instead rolls everything back
///     Ok(user)
/// }
/// ```
pub trait Database: Send + Sync + 'static {
    type Connection: Send;
    /// A unit of work: everything done through it commits or rolls back together.
    type Transaction: Transaction;

    fn connection(&self) -> impl Future<Output = Result<Self::Connection, StorageError>> + Send;
    /// Starts a transaction. Reads that must not race with a following write go through
    /// [`Repository::find_by_id_for_update`](crate::repository::Repository::find_by_id_for_update).
    fn transaction(&self) -> impl Future<Output = Result<Self::Transaction, StorageError>> + Send;
    fn ping(&self) -> impl Future<Output = Result<(), StorageError>> + Send;
}

pub trait Transaction: Send {
    fn commit(self) -> impl Future<Output = Result<(), StorageError>> + Send;
}
