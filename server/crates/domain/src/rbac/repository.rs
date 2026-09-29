use std::future::Future;

use crate::{
    error::StorageError,
    rbac::{Permission, PermissionSet, Role, RoleName},
    user::UserId,
};

#[diagnostic::on_unimplemented(
    message = "`{Self}` does not implement `RbacRepository`",
    note = "every port a service uses must be part of `application::Store` (context.rs: the trait and its blanket impl) and implemented for `PgExecutor<C>` in infrastructure/src/db/repositories/"
)]
pub trait RbacRepository: Send {
    fn list_roles(&mut self) -> impl Future<Output = Result<Vec<Role>, StorageError>> + Send;

    fn find_role(
        &mut self,
        name: &RoleName,
    ) -> impl Future<Output = Result<Option<Role>, StorageError>> + Send;

    fn roles_of_users(
        &mut self,
        users: &[UserId],
    ) -> impl Future<Output = Result<Vec<(UserId, RoleName)>, StorageError>> + Send;

    fn permissions_of_user(
        &mut self,
        user: UserId,
    ) -> impl Future<Output = Result<PermissionSet, StorageError>> + Send;

    /// Returns whether the user did not hold the role yet. Fails with a foreign key violation if
    /// the role does not exist.
    fn grant_role(
        &mut self,
        user: UserId,
        role: &RoleName,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send;

    fn revoke_role(
        &mut self,
        user: UserId,
        role: &RoleName,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send;

    fn count_enabled_users_with(
        &mut self,
        permission: Permission,
    ) -> impl Future<Output = Result<u64, StorageError>> + Send;

    fn lock_role_assignments(&mut self) -> impl Future<Output = Result<(), StorageError>> + Send;
}
