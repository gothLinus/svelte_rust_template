use std::{collections::HashMap, sync::Arc};

use domain::{
    audit::{AuditAction, AuditRepository, NewAuditEvent},
    clock::Clock,
    database::{Database, Transaction},
    error::StorageError,
    i18n::{Locale, Message},
    rbac::{Permission, RbacRepository, Role, RoleName},
    session::{SessionId, SessionRepository},
    user::{User, UserId, UserRepository},
};

use crate::{
    Adapters, Context,
    actor::Actor,
    admin::{AccountStatus, AdminPolicy, dto::ListUsersQuery, ensure_someone_manages_users},
    auth::access,
    dto::{RoleDto, SessionDto, UserDto},
    error::AppError,
    pagination::PageDto,
};

/// The account administration use cases. Every method checks [`AdminPolicy`] first; each change
/// runs in one transaction that holds `lock_role_assignments`, so concurrent changes cannot each
/// leave the system without a user manager.
pub struct AdminService<A: Adapters> {
    ctx: Arc<Context<A>>,
}

impl<A: Adapters> AdminService<A> {
    pub fn new(ctx: Arc<Context<A>>) -> Self {
        Self { ctx }
    }

    pub async fn list_users(
        &self,
        actor: &Actor,
        query: ListUsersQuery,
    ) -> Result<PageDto<UserDto>, AppError> {
        AdminPolicy::can_view_users(actor)?;
        let (filter, page) = query.try_into()?;

        let mut conn = self.ctx.db.connection().await?;
        let users = conn.list_users(&filter, page).await?;
        let ids: Vec<UserId> = users.items.iter().map(User::id).collect();
        let mut roles: HashMap<UserId, Vec<RoleName>> = HashMap::new();
        for (user, role) in conn.roles_of_users(&ids).await? {
            roles.entry(user).or_default().push(role);
        }

        Ok(users
            .map(|user| {
                let roles = roles.get(&user.id()).map_or(&[][..], Vec::as_slice);
                UserDto::new(&user, roles)
            })
            .into())
    }

    pub async fn get_user(&self, actor: &Actor, id: UserId) -> Result<UserDto, AppError> {
        AdminPolicy::can_view_users(actor)?;
        let mut conn = self.ctx.db.connection().await?;
        let user = conn.find_user(id).await?.ok_or(AppError::NotFound)?;
        Ok(user_dto(&mut conn, &user).await?)
    }

    /// Every role, with the built-in ones described in `locale`; roles added since keep the
    /// description stored with them.
    pub async fn list_roles(
        &self,
        actor: &Actor,
        locale: &Locale,
    ) -> Result<Vec<RoleDto>, AppError> {
        AdminPolicy::can_view_users(actor)?;
        let roles = self.ctx.db.connection().await?.list_roles().await?;
        Ok(roles
            .into_iter()
            .map(|role| self.describe(role, locale))
            .collect())
    }

    fn describe(&self, role: Role, locale: &Locale) -> RoleDto {
        let description = if role.name == RoleName::ADMIN {
            Some(Message::new("role-admin-description"))
        } else if role.name == RoleName::USER {
            Some(Message::new("role-user-description"))
        } else {
            None
        };
        let mut dto = RoleDto::from(role);
        if let Some(description) = description {
            dto.description = self.ctx.translator.translate(locale, &description);
        }
        dto
    }

    /// Gives the user a role. Their sessions get a new token on their next request, since their
    /// privileges changed.
    ///
    /// # Errors
    ///
    /// `Forbidden` without `users:manage`, or if the role carries a permission the actor lacks;
    /// `ReauthRequired` without a recent sign-in; `NotFound` if the user or role does not exist (a
    /// malformed role name counts as unknown).
    pub async fn grant_role(
        &self,
        actor: &Actor,
        id: UserId,
        role: &str,
    ) -> Result<UserDto, AppError> {
        AdminPolicy::can_change_roles(actor)?;
        let role = RoleName::parse(role).map_err(|_| AppError::NotFound)?;

        let mut tx = self.ctx.db.transaction().await?;
        tx.lock_role_assignments().await?;
        let granted = tx.find_role(&role).await?.ok_or(AppError::NotFound)?;
        AdminPolicy::can_assign(actor, granted.permissions)?;
        let user = tx
            .find_user_for_update(id)
            .await?
            .ok_or(AppError::NotFound)?;
        match tx.grant_role(id, &role).await {
            Ok(true) => {
                tx.flag_user_sessions_for_rotation(id).await?;
                tx.record_audit_event(
                    &self
                        .admin_event(actor, id, AuditAction::RoleGranted)
                        .detail(role.as_str()),
                )
                .await?;
            }
            Ok(false) => {}
            Err(StorageError::ForeignKeyViolation { .. }) => return Err(AppError::NotFound),
            Err(err) => return Err(err.into()),
        }
        let dto = user_dto(&mut tx, &user).await?;
        tx.commit().await?;

        tracing::info!(actor = %actor.user_id, user_id = %id, %role, "role granted");
        Ok(dto)
    }

    /// Takes a role away. Refused if nobody enabled would be left to manage users.
    ///
    /// # Errors
    ///
    /// As [`AdminService::grant_role`], and `Conflict` (`last_admin`) if it would leave no enabled
    /// account able to manage users.
    pub async fn revoke_role(
        &self,
        actor: &Actor,
        id: UserId,
        role: &str,
    ) -> Result<UserDto, AppError> {
        AdminPolicy::can_change_roles(actor)?;
        let role = RoleName::parse(role).map_err(|_| AppError::NotFound)?;

        let mut tx = self.ctx.db.transaction().await?;
        tx.lock_role_assignments().await?;
        let revoked = tx.find_role(&role).await?.ok_or(AppError::NotFound)?;
        AdminPolicy::can_assign(actor, revoked.permissions)?;
        let user = tx
            .find_user_for_update(id)
            .await?
            .ok_or(AppError::NotFound)?;
        let could_manage = tx
            .permissions_of_user(id)
            .await?
            .contains(Permission::UsersManage);
        if tx.revoke_role(id, &role).await? {
            if could_manage {
                ensure_someone_manages_users(&mut tx).await?;
            }
            tx.flag_user_sessions_for_rotation(id).await?;
            tx.record_audit_event(
                &self
                    .admin_event(actor, id, AuditAction::RoleRevoked)
                    .detail(role.as_str()),
            )
            .await?;
        }
        let dto = user_dto(&mut tx, &user).await?;
        tx.commit().await?;

        tracing::info!(actor = %actor.user_id, user_id = %id, %role, "role revoked");
        Ok(dto)
    }

    /// Disables or re-enables an account. Disabling signs the user out everywhere.
    ///
    /// # Errors
    ///
    /// `Forbidden` without `users:manage`, or if the account holds a permission the actor lacks
    /// (this applies to re-enabling too); `ReauthRequired` without a recent sign-in; `Conflict`
    /// (`cannot_disable_self`) for the actor's own account, and (`last_admin`) if disabling would
    /// leave no enabled account able to manage users; `NotFound` if there is no such user.
    pub async fn set_status(
        &self,
        actor: &Actor,
        id: UserId,
        status: AccountStatus,
    ) -> Result<UserDto, AppError> {
        AdminPolicy::can_set_disabled(actor, id)?;
        let disabled = status == AccountStatus::Disabled;
        let at = disabled.then(|| self.ctx.clock.now());

        let mut tx = self.ctx.db.transaction().await?;
        tx.lock_role_assignments().await?;
        let held = tx.permissions_of_user(id).await?;
        AdminPolicy::can_set_disabled_holder_of(actor, held)?;
        let was_disabled = tx
            .find_user_for_update(id)
            .await?
            .ok_or(AppError::NotFound)?
            .is_disabled();
        let user = tx
            .set_user_disabled(id, at)
            .await?
            .ok_or(AppError::NotFound)?;
        if was_disabled != disabled {
            let action = if disabled {
                AuditAction::AccountDisabled
            } else {
                AuditAction::AccountEnabled
            };
            tx.record_audit_event(&self.admin_event(actor, id, action))
                .await?;
        }
        if disabled {
            access::revoke_all(&mut tx, id, None).await?;
            if held.contains(Permission::UsersManage) {
                ensure_someone_manages_users(&mut tx).await?;
            }
        }
        let dto = user_dto(&mut tx, &user).await?;
        tx.commit().await?;

        tracing::info!(actor = %actor.user_id, user_id = %id, ?status, "account status changed");
        Ok(dto)
    }

    /// The user's active sessions, most recently used first. `current` marks the actor's own.
    ///
    /// # Errors
    ///
    /// `Forbidden` without `users:read`; `NotFound` if there is no such user.
    pub async fn user_sessions(
        &self,
        actor: &Actor,
        id: UserId,
    ) -> Result<Vec<SessionDto>, AppError> {
        AdminPolicy::can_view_users(actor)?;
        let now = self.ctx.clock.now();
        let policy = &self.ctx.settings.sessions;

        let mut conn = self.ctx.db.connection().await?;
        conn.find_user(id).await?.ok_or(AppError::NotFound)?;
        let sessions = conn.list_user_sessions(id).await?;
        Ok(sessions
            .iter()
            .filter(|session| session.is_active(now, policy))
            .map(|session| SessionDto::new(session, session.id() == actor.session_id, policy))
            .collect())
    }

    /// Ends one of the user's sessions.
    ///
    /// # Errors
    ///
    /// `Forbidden` without `users:manage`, or if the account holds a permission the actor lacks;
    /// `ReauthRequired` without a recent sign-in; `NotFound` if the user has no such session.
    pub async fn revoke_user_session(
        &self,
        actor: &Actor,
        id: UserId,
        session: SessionId,
    ) -> Result<(), AppError> {
        AdminPolicy::can_revoke_sessions(actor)?;

        let mut tx = self.ctx.db.transaction().await?;
        let held = tx.permissions_of_user(id).await?;
        AdminPolicy::can_set_disabled_holder_of(actor, held)?;
        if !tx.delete_user_session(id, session).await? {
            return Err(AppError::NotFound);
        }
        tx.record_audit_event(&self.admin_event(actor, id, AuditAction::SessionRevoked))
            .await?;
        tx.commit().await?;

        tracing::info!(actor = %actor.user_id, user_id = %id, session_id = %session, "session revoked by admin");
        Ok(())
    }

    /// Signs the user out everywhere without disabling the account: every session ends, and so do
    /// pending links and codes, so a stolen session or link stops working at once.
    ///
    /// # Errors
    ///
    /// As [`AdminService::revoke_user_session`], with `NotFound` if there is no such user.
    pub async fn sign_out_user(&self, actor: &Actor, id: UserId) -> Result<(), AppError> {
        AdminPolicy::can_revoke_sessions(actor)?;

        let mut tx = self.ctx.db.transaction().await?;
        tx.find_user_for_update(id)
            .await?
            .ok_or(AppError::NotFound)?;
        let held = tx.permissions_of_user(id).await?;
        AdminPolicy::can_set_disabled_holder_of(actor, held)?;
        let revoked = access::revoke_all(&mut tx, id, None).await?;
        tx.record_audit_event(&self.admin_event(actor, id, AuditAction::SignedOutEverywhere))
            .await?;
        tx.commit().await?;

        tracing::info!(actor = %actor.user_id, user_id = %id, revoked, "user signed out everywhere by admin");
        Ok(())
    }

    /// An event about `user` caused by the administrator `actor`.
    fn admin_event(&self, actor: &Actor, user: UserId, action: AuditAction) -> NewAuditEvent {
        self.ctx
            .event(user, action, &actor.client)
            .by(actor.user_id)
    }
}

async fn user_dto(store: &mut impl RbacRepository, user: &User) -> Result<UserDto, StorageError> {
    let roles: Vec<RoleName> = store
        .roles_of_users(&[user.id()])
        .await?
        .into_iter()
        .map(|(_, role)| role)
        .collect();
    Ok(UserDto::new(user, &roles))
}
