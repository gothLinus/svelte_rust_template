//! `RbacRepository`: roles, their permissions and role assignments.
//!
//! Permissions are stored by name. A name this build does not know is skipped with a warning
//! instead of failing the request, so a rolling deploy can add one safely.
//! `lock_role_assignments` takes a transaction-scoped advisory lock, so concurrent role changes
//! cannot each see the other administrator as the one who remains.

use domain::{
    error::StorageError,
    rbac::{Permission, PermissionSet, RbacRepository, Role, RoleName},
    user::UserId,
};
use uuid::Uuid;

use crate::db::{
    errors::{corrupt, db_error, to_u64},
    postgres::{PgExecutor, PgHandle},
};

const ROLE_ASSIGNMENT_LOCK: i64 = 0x726f_6c65_7300;

struct RoleRow {
    name: String,
    description: String,
    permissions: Vec<String>,
}

impl TryFrom<RoleRow> for Role {
    type Error = StorageError;

    fn try_from(row: RoleRow) -> Result<Self, Self::Error> {
        Ok(Self {
            name: RoleName::parse(&row.name).map_err(corrupt)?,
            description: row.description,
            permissions: permission_set(&row.permissions),
        })
    }
}

/// Parses permission names from the database. Names this build does not know (added by a newer
/// migration during a rolling deploy) are skipped with a warning rather than failing every
/// request; the permission-sync test catches real drift.
pub(super) fn permission_set(names: &[String]) -> PermissionSet {
    names
        .iter()
        .filter_map(|name| {
            Permission::parse(name)
                .inspect_err(|_| tracing::warn!(permission = %name, "ignoring unknown permission"))
                .ok()
        })
        .collect()
}

impl<C: PgHandle> RbacRepository for PgExecutor<C> {
    async fn list_roles(&mut self) -> Result<Vec<Role>, StorageError> {
        sqlx::query_as!(
            RoleRow,
            r#"
            select r.name, r.description,
                coalesce(
                    array_agg(rp.permission order by rp.permission)
                        filter (where rp.permission is not null),
                    '{}'
                ) as "permissions!"
            from roles r
            left join role_permissions rp on rp.role = r.name
            group by r.name
            order by r.name
            "#
        )
        .fetch_all(self.conn())
        .await
        .map_err(db_error)?
        .into_iter()
        .map(Role::try_from)
        .collect()
    }

    async fn find_role(&mut self, name: &RoleName) -> Result<Option<Role>, StorageError> {
        sqlx::query_as!(
            RoleRow,
            r#"
            select r.name, r.description,
                coalesce(
                    array_agg(rp.permission order by rp.permission)
                        filter (where rp.permission is not null),
                    '{}'
                ) as "permissions!"
            from roles r
            left join role_permissions rp on rp.role = r.name
            where r.name = $1
            group by r.name
            "#,
            name.as_str(),
        )
        .fetch_optional(self.conn())
        .await
        .map_err(db_error)?
        .map(Role::try_from)
        .transpose()
    }

    async fn roles_of_users(
        &mut self,
        users: &[UserId],
    ) -> Result<Vec<(UserId, RoleName)>, StorageError> {
        let ids: Vec<Uuid> = users.iter().map(UserId::as_uuid).collect();
        sqlx::query!(
            r#"
            select user_id, role
            from user_roles
            where user_id = any($1)
            order by user_id, role
            "#,
            &ids,
        )
        .fetch_all(self.conn())
        .await
        .map_err(db_error)?
        .into_iter()
        .map(|row| {
            let role = RoleName::parse(&row.role).map_err(corrupt)?;
            Ok((UserId::from_uuid(row.user_id), role))
        })
        .collect()
    }

    async fn permissions_of_user(&mut self, user: UserId) -> Result<PermissionSet, StorageError> {
        let names = sqlx::query_scalar!(
            r#"
            select distinct rp.permission
            from user_roles ur
            join role_permissions rp on rp.role = ur.role
            where ur.user_id = $1
            "#,
            user.as_uuid(),
        )
        .fetch_all(self.conn())
        .await
        .map_err(db_error)?;

        Ok(permission_set(&names))
    }

    async fn grant_role(&mut self, user: UserId, role: &RoleName) -> Result<bool, StorageError> {
        let result = sqlx::query!(
            r#"
            insert into user_roles (user_id, role) values ($1, $2)
            on conflict (user_id, role) do nothing
            "#,
            user.as_uuid(),
            role.as_str(),
        )
        .execute(self.conn())
        .await
        .map_err(db_error)?;

        Ok(result.rows_affected() > 0)
    }

    async fn revoke_role(&mut self, user: UserId, role: &RoleName) -> Result<bool, StorageError> {
        let result = sqlx::query!(
            "delete from user_roles where user_id = $1 and role = $2",
            user.as_uuid(),
            role.as_str(),
        )
        .execute(self.conn())
        .await
        .map_err(db_error)?;

        Ok(result.rows_affected() > 0)
    }

    async fn count_enabled_users_with(
        &mut self,
        permission: Permission,
    ) -> Result<u64, StorageError> {
        let count = sqlx::query_scalar!(
            r#"
            select count(distinct u.id) as "count!"
            from users u
            join user_roles ur on ur.user_id = u.id
            join role_permissions rp on rp.role = ur.role
            where rp.permission = $1 and u.disabled_at is null
            "#,
            permission.as_str(),
        )
        .fetch_one(self.conn())
        .await
        .map_err(db_error)?;

        to_u64(count)
    }

    async fn lock_role_assignments(&mut self) -> Result<(), StorageError> {
        // Transaction-scoped: released on commit or rollback. Outside a transaction it would be
        // released immediately, which makes it useless but harmless.
        sqlx::query!(
            r#"select true as "locked!" from (select pg_advisory_xact_lock($1)) as lock"#,
            ROLE_ASSIGNMENT_LOCK,
        )
        .fetch_one(self.conn())
        .await
        .map_err(db_error)?;

        Ok(())
    }
}
