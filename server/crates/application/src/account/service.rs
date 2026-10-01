use std::sync::Arc;

use domain::{
    audit::{AuditAction, AuditRepository},
    clock::Clock,
    database::{Database, Transaction},
    error::ValidationError,
    i18n::{Locale, Message},
    identity::IdentityRepository,
    mfa::MfaRepository,
    one_time_code::{CODE_TTL, CodeChannel, CodePurpose},
    passkey::PasskeyRepository,
    rbac::{Permission, RbacRepository},
    security::{PasswordHasher, TokenGenerator},
    session::{SessionId, SessionRepository},
    user::{
        Email, PHONE_UNIQUE_CONSTRAINT, PhoneNumber, USERNAME_UNIQUE_CONSTRAINT, User,
        UserRepository,
    },
    user_token::{TokenPurpose, UserToken, UserTokenRepository},
};

use crate::{
    Adapters, Context,
    account::{
        dto::{
            AddPhoneRequest, ChangeEmailRequest, DeleteAccountRequest, ProfileChange, SecurityDto,
            SetLocaleRequest, UpdateProfileRequest,
        },
        load_me,
    },
    actor::Actor,
    admin::ensure_someone_manages_users,
    auth::signin::second_factors,
    codes,
    dto::{MeDto, MfaMethod, SessionDto},
    error::AppError,
    mail,
    mfa::dto::CodeRequest,
    oauth::dto::IdentityDto,
    passkeys::dto::PasskeyDto,
    passwordless::{group, send_text},
};

pub struct AccountService<A: Adapters> {
    ctx: Arc<Context<A>>,
}

impl<A: Adapters> AccountService<A> {
    pub fn new(ctx: Arc<Context<A>>) -> Self {
        Self { ctx }
    }

    pub async fn me(&self, user: &User) -> Result<MeDto, AppError> {
        let mut conn = self.ctx.db.connection().await?;
        Ok(load_me(&mut conn, user).await?)
    }

    pub async fn update_profile(
        &self,
        actor: &Actor,
        request: UpdateProfileRequest,
    ) -> Result<MeDto, AppError> {
        let change = ProfileChange::try_from(request)?;
        let mut tx = self.ctx.db.transaction().await?;
        let previous = tx
            .find_user_for_update(actor.user_id)
            .await?
            .ok_or(AppError::Unauthenticated)?;
        let user = match tx.set_user_username(actor.user_id, &change.username).await {
            Ok(user) => user.ok_or(AppError::Unauthenticated)?,
            Err(err) if err.is_unique_violation(USERNAME_UNIQUE_CONSTRAINT) => {
                return Err(crate::error::username_taken());
            }
            Err(err) => return Err(err.into()),
        };
        if previous.username() != user.username() {
            tx.record_audit_event(
                &self
                    .ctx
                    .actor_event(actor, AuditAction::UsernameChanged)
                    .detail(user.username().as_str()),
            )
            .await?;
        }
        let me = load_me(&mut tx, &user).await?;
        tx.commit().await?;
        Ok(me)
    }

    /// Sets the language mail and texts to the actor are written in.
    ///
    /// # Errors
    ///
    /// `Validation` on `locale` (`unsupported_locale`) for a language the catalog does not have.
    pub async fn set_locale(
        &self,
        actor: &Actor,
        request: SetLocaleRequest,
    ) -> Result<MeDto, AppError> {
        let locale = match request.locale.as_deref().map(str::trim) {
            None | Some("") => None,
            Some(tag) => Some(
                Locale::parse(tag)
                    .filter(|locale| self.ctx.translator.locales().contains(locale))
                    .ok_or_else(|| {
                        AppError::invalid(
                            "locale",
                            &ValidationError::new(
                                "unsupported_locale",
                                Message::new("validation-locale-unsupported"),
                            ),
                        )
                    })?,
            ),
        };
        let mut conn = self.ctx.db.connection().await?;
        let user = conn
            .set_user_locale(actor.user_id, locale.as_ref())
            .await?
            .ok_or(AppError::Unauthenticated)?;
        Ok(load_me(&mut conn, &user).await?)
    }

    /// Mails a link to the new address; the change happens when it is opened
    /// ([`AuthService::confirm_email_change`](crate::auth::AuthService::confirm_email_change)). The
    /// current address gets a notice with a link that cancels the change and signs out every
    /// device. A pending request of the same kind is replaced.
    ///
    /// # Errors
    ///
    /// `ReauthRequired` without a recent sign-in. `Validation` on `email` if it is malformed or the
    /// current address; `Conflict` (`email_taken`) if another account uses it.
    pub async fn request_email_change(
        &self,
        actor: &Actor,
        request: ChangeEmailRequest,
    ) -> Result<(), AppError> {
        actor.require_recent_authentication()?;
        let email = Email::parse(&request.email).map_err(|err| AppError::invalid("email", &err))?;
        let mut conn = self.ctx.db.connection().await?;
        let user = conn
            .find_user(actor.user_id)
            .await?
            .ok_or(AppError::Unauthenticated)?;
        if user.email() == &email {
            return Err(AppError::invalid(
                "email",
                &ValidationError::new("unchanged", Message::new("validation-email-unchanged")),
            ));
        }
        if conn.find_user_by_email(&email).await?.is_some() {
            return Err(AppError::conflict(
                "email_taken",
                Message::new("conflict-email-taken"),
            ));
        }

        let now = self.ctx.clock.now();
        let token = self.ctx.tokens.generate()?;
        let cancel = self.ctx.tokens.generate()?;
        conn.replace_user_token(&UserToken::email_change(
            user.id(),
            email.clone(),
            self.ctx.tokens.digest(&token),
            now,
            &self.ctx.settings.tokens,
        ))
        .await?;
        conn.replace_user_token(&UserToken::email_change_cancel(
            user.id(),
            user.email().clone(),
            self.ctx.tokens.digest(&cancel),
            now,
            &self.ctx.settings.tokens,
        ))
        .await?;
        drop(conn);

        let links = &self.ctx.settings.links;
        let ttl = self.ctx.settings.tokens.email_change_ttl;
        mail::send(
            &*self.ctx.mailer,
            mail::confirm_email_change(
                &self.ctx.voice_for(&user),
                email.clone(),
                &links.confirm_email(&token),
                ttl,
            ),
        )
        .await;
        // The old address can stop, or later undo, a change it did not ask for.
        mail::send(
            &*self.ctx.mailer,
            mail::email_change_requested(
                &self.ctx.voice_for(&user),
                user.email().clone(),
                &email.masked(),
                &links.cancel_email_change(&cancel),
            ),
        )
        .await;
        tracing::info!(user_id = %user.id(), "email change requested");
        Ok(())
    }

    /// Mails a link to choose a new password. Asking for the old password instead would let anyone
    /// with a stolen session lock the owner out; the link needs the mailbox, so this needs no
    /// recent sign-in itself.
    pub async fn request_password_change(&self, actor: &Actor) -> Result<(), AppError> {
        let mut conn = self.ctx.db.connection().await?;
        let user = conn
            .find_user(actor.user_id)
            .await?
            .ok_or(AppError::Unauthenticated)?;
        let token = crate::tokens::issue(
            &self.ctx,
            &mut conn,
            user.id(),
            TokenPurpose::PasswordReset,
            self.ctx.clock.now(),
        )
        .await?;
        drop(conn);

        let link = self.ctx.settings.links.reset_password(&token);
        let ttl = self.ctx.settings.tokens.password_reset_ttl;
        mail::send(
            &*self.ctx.mailer,
            mail::password_reset(&self.ctx.voice_for(&user), user.email().clone(), &link, ttl),
        )
        .await;
        tracing::info!(user_id = %user.id(), "password change link sent");
        Ok(())
    }

    /// Texts a code to a new phone number. The number is saved once the code comes back
    /// ([`AccountService::verify_phone`]).
    ///
    /// # Errors
    ///
    /// `ReauthRequired` without a recent sign-in; `Conflict` (`email_unverified`) while the email
    /// address is unverified, and (`channel_unavailable`) if the channel is not set up.
    /// `Validation` on `phone` if the number is malformed, from an unsupported country or used by
    /// another account.
    pub async fn add_phone(&self, actor: &Actor, request: AddPhoneRequest) -> Result<(), AppError> {
        actor.require_verified_email()?;
        actor.require_recent_authentication()?;
        let channel = CodeChannel::from(request.channel);
        if !self.ctx.texts.channels().contains(&channel) {
            return Err(AppError::conflict(
                "channel_unavailable",
                Message::new("conflict-channel-unavailable").arg("channel", channel.as_str()),
            ));
        }
        let phone =
            PhoneNumber::parse(&request.phone).map_err(|err| AppError::invalid("phone", &err))?;
        self.ctx.settings.ensure_textable(&phone)?;

        let mut conn = self.ctx.db.connection().await?;
        if conn
            .find_user_by_phone(&phone)
            .await?
            .is_some_and(|owner| owner.id() != actor.user_id)
        {
            return Err(phone_taken());
        }
        let user = conn
            .find_user(actor.user_id)
            .await?
            .ok_or(AppError::Unauthenticated)?;
        let code = codes::issue(
            &self.ctx,
            &mut conn,
            actor.user_id,
            CodePurpose::PhoneVerification,
            channel,
            Some(phone.clone()),
        )
        .await?;
        drop(conn);

        let message = Message::new("sms-phone-verification-code")
            .arg("code", group(code.expose()))
            .arg("app", self.ctx.settings.app_name.as_str())
            .arg("minutes", CODE_TTL.whole_minutes());
        send_text(&self.ctx, &user, phone, channel, &message).await;
        Ok(())
    }

    /// Saves the number the code was sent to, as verified. A wrong code is a `Validation` error on
    /// `code` and still counts towards the attempt limit. Same guards as
    /// [`AccountService::add_phone`].
    pub async fn verify_phone(
        &self,
        actor: &Actor,
        request: CodeRequest,
    ) -> Result<MeDto, AppError> {
        actor.require_verified_email()?;
        actor.require_recent_authentication()?;

        let mut tx = self.ctx.db.transaction().await?;
        let checked = codes::check(
            &self.ctx,
            &mut tx,
            actor.user_id,
            CodePurpose::PhoneVerification,
            &[CodeChannel::Sms, CodeChannel::Whatsapp],
            request.code.0.expose(),
        )
        .await;
        let code = match checked {
            Ok(code) => code,
            Err(err) => {
                tx.commit().await?;
                return Err(err);
            }
        };
        let phone = code.target.ok_or_else(AppError::invalid_code)?;
        let user = match tx
            .set_user_phone(actor.user_id, Some((&phone, self.ctx.clock.now())))
            .await
        {
            Ok(user) => user.ok_or(AppError::Unauthenticated)?,
            Err(err) if err.is_unique_violation(PHONE_UNIQUE_CONSTRAINT) => {
                return Err(phone_taken());
            }
            Err(err) => return Err(err.into()),
        };
        tx.record_audit_event(
            &self
                .ctx
                .actor_event(actor, AuditAction::PhoneAdded)
                .detail(phone.masked()),
        )
        .await?;
        let me = load_me(&mut tx, &user).await?;
        tx.commit().await?;
        tracing::info!(user_id = %actor.user_id, "phone number verified");
        Ok(me)
    }

    pub async fn remove_phone(&self, actor: &Actor) -> Result<MeDto, AppError> {
        actor.require_recent_authentication()?;
        let mut tx = self.ctx.db.transaction().await?;
        let had_phone = tx
            .find_user_for_update(actor.user_id)
            .await?
            .ok_or(AppError::Unauthenticated)?
            .phone()
            .is_some();
        let user = tx
            .set_user_phone(actor.user_id, None)
            .await?
            .ok_or(AppError::Unauthenticated)?;
        if had_phone {
            tx.record_audit_event(&self.ctx.actor_event(actor, AuditAction::PhoneRemoved))
                .await?;
        }
        let me = load_me(&mut tx, &user).await?;
        tx.commit().await?;
        Ok(me)
    }

    pub async fn security(&self, actor: &Actor) -> Result<SecurityDto, AppError> {
        let mut conn = self.ctx.db.connection().await?;
        let user = conn
            .find_user(actor.user_id)
            .await?
            .ok_or(AppError::Unauthenticated)?;
        let methods = second_factors(&mut conn, user.id()).await?;
        let passkeys = conn.list_user_passkeys(user.id()).await?;
        let identities = conn.list_user_identities(user.id()).await?;
        let recovery_codes = conn.count_recovery_codes(user.id()).await?;
        let providers = self.ctx.identity_providers.providers();

        Ok(SecurityDto {
            has_password: user.has_password(),
            mfa_enabled: !methods.is_empty(),
            totp_enabled: methods.contains(&MfaMethod::Totp),
            recovery_codes_remaining: u32::try_from(recovery_codes).unwrap_or(u32::MAX),
            passkeys: passkeys.iter().map(PasskeyDto::from).collect(),
            linked_accounts: identities
                .iter()
                .map(|identity| {
                    let name = providers
                        .iter()
                        .find(|provider| provider.id == identity.provider)
                        .map_or(identity.provider.as_str(), |provider| {
                            provider.name.as_str()
                        });
                    IdentityDto::new(identity, name)
                })
                .collect(),
        })
    }
    /// Everything stored about the actor, for their right of access and to data portability. Needs
    /// a recent sign-in (`ReauthRequired`) because it is all of their data at once. Tokens, secrets
    /// and digests are left out; see [`crate::export`].
    pub async fn export(&self, actor: &Actor) -> Result<serde_json::Value, AppError> {
        use serde_json::json;

        use crate::export::{Export, timestamp};

        actor.require_recent_authentication()?;
        let mut conn = self.ctx.db.connection().await?;
        let user = conn
            .find_user(actor.user_id)
            .await?
            .ok_or(AppError::Unauthenticated)?;
        let id = user.id();
        let mut export = Export::new(self.ctx.clock.now());

        let roles: Vec<String> = conn
            .roles_of_users(&[id])
            .await?
            .into_iter()
            .map(|(_, role)| role.to_string())
            .collect();
        export.add(
            "account",
            json!({
                "id": id.to_string(),
                "email": user.email().as_str(),
                "email_verified_at": user.email_verified_at().and_then(timestamp),
                "username": user.username().as_str(),
                "phone": user.phone().map(domain::user::PhoneNumber::as_str),
                "phone_verified_at": user.phone_verified_at().and_then(timestamp),
                "has_password": user.has_password(),
                "roles": roles,
                "created_at": timestamp(user.created_at()),
                "updated_at": timestamp(user.updated_at()),
            }),
        );
        let sessions: Vec<_> = conn
            .list_user_sessions(id)
            .await?
            .iter()
            .map(|session| {
                json!({
                    "signed_in_at": timestamp(session.created_at()),
                    "last_seen_at": timestamp(session.last_seen_at()),
                    "expires_at": timestamp(session.expires_at()),
                    "ip": session.client().ip.map(|ip| ip.to_string()),
                    "user_agent": session.client().user_agent,
                })
            })
            .collect();
        export.add("sessions", json!(sessions));
        let identities: Vec<_> = conn
            .list_user_identities(id)
            .await?
            .iter()
            .map(|identity| {
                json!({
                    "provider": identity.provider,
                    "provider_account_id": identity.subject,
                    "email": identity.email,
                    "linked_at": timestamp(identity.created_at),
                    "last_used_at": timestamp(identity.last_used_at),
                })
            })
            .collect();
        export.add("linked_accounts", json!(identities));
        let passkeys: Vec<_> = conn
            .list_user_passkeys(id)
            .await?
            .iter()
            .map(|passkey| {
                json!({
                    "name": passkey.name.as_str(),
                    "transports": passkey.transports,
                    "created_at": timestamp(passkey.created_at),
                    "last_used_at": passkey.last_used_at.and_then(timestamp),
                })
            })
            .collect();
        export.add("passkeys", json!(passkeys));
        let totp = conn.find_totp(id).await?;
        export.add(
            "authenticator_app",
            json!({
                "enabled": totp.as_ref().is_some_and(domain::mfa::TotpCredential::is_confirmed),
                "enabled_at": totp.and_then(|totp| totp.confirmed_at).and_then(timestamp),
            }),
        );
        export.add(
            "recovery_codes_remaining",
            json!(conn.count_recovery_codes(id).await?),
        );

        export.add("security_activity", audit_events(&mut conn, id).await?);

        export
            .add_owned::<domain::note::Note>(&mut conn, id)
            .await?;

        drop(conn);
        tracing::info!(user_id = %id, "personal data exported");
        Ok(export.into_json())
    }

    pub async fn sessions(&self, actor: &Actor) -> Result<Vec<SessionDto>, AppError> {
        let now = self.ctx.clock.now();
        let policy = &self.ctx.settings.sessions;
        let sessions = self
            .ctx
            .db
            .connection()
            .await?
            .list_user_sessions(actor.user_id)
            .await?;

        Ok(sessions
            .iter()
            .filter(|session| session.is_active(now, policy))
            .map(|session| SessionDto::new(session, session.id() == actor.session_id, policy))
            .collect())
    }

    pub async fn revoke_session(&self, actor: &Actor, id: SessionId) -> Result<(), AppError> {
        let mut tx = self.ctx.db.transaction().await?;
        if !tx.delete_user_session(actor.user_id, id).await? {
            return Err(AppError::NotFound);
        }
        tx.record_audit_event(&self.ctx.actor_event(actor, AuditAction::SessionRevoked))
            .await?;
        tx.commit().await?;
        Ok(())
    }

    /// Deletes the account and everything it owns, and refuses if it would leave nobody able to
    /// manage users. Needs a recent sign-in or re-authentication, or else the account's password in
    /// the request.
    ///
    /// # Errors
    ///
    /// `Validation` on `password` (`incorrect_password`) if a password was sent and is wrong, even
    /// within the re-authentication window. `ReauthRequired` if none was sent, or the account has
    /// none, and the session is not recent. `Conflict` (`last_admin`) if the user is the last
    /// enabled account that can manage users; the deletion is then rolled back.
    pub async fn delete_account(
        &self,
        actor: &Actor,
        request: DeleteAccountRequest,
    ) -> Result<(), AppError> {
        let user = self
            .ctx
            .db
            .connection()
            .await?
            .find_user(actor.user_id)
            .await?
            .ok_or(AppError::Unauthenticated)?;
        let password = request
            .password
            .map(|password| password.0)
            .filter(|password| !password.is_empty() && user.has_password());
        match password {
            // A password that was sent is checked, even within the re-authentication window:
            // someone typed it to confirm.
            Some(password) => {
                if !self
                    .ctx
                    .hasher
                    .verify(&password, user.password_hash())
                    .await?
                {
                    return Err(AppError::invalid(
                        "password",
                        &ValidationError::new(
                            "incorrect_password",
                            Message::new("validation-incorrect-password"),
                        ),
                    ));
                }
            }
            None => actor.require_recent_authentication()?,
        }

        let mut tx = self.ctx.db.transaction().await?;
        tx.lock_role_assignments().await?;
        tx.delete_user(user.id()).await?;
        if actor.has(Permission::UsersManage) {
            ensure_someone_manages_users(&mut tx).await?;
        }
        tx.commit().await?;

        tracing::info!(user_id = %user.id(), "account deleted");
        Ok(())
    }
}

/// Every audit event about `user`, newest first, for the export.
async fn audit_events(
    store: &mut impl AuditRepository,
    user: domain::user::UserId,
) -> Result<serde_json::Value, AppError> {
    use domain::{
        audit::AuditFilter,
        pagination::{MAX_PAGE_SIZE, PageRequest, PageSize},
    };
    use serde_json::json;

    use crate::export::timestamp;

    let filter = AuditFilter {
        user_id: Some(user),
    };
    let size =
        PageSize::parse(Some(MAX_PAGE_SIZE)).map_err(|err| AppError::invalid("limit", &err))?;
    let mut events = Vec::new();
    let mut after = None;
    loop {
        let page = store
            .list_audit_events(&filter, PageRequest::new(size, after.take()))
            .await?;
        events.extend(page.items.iter().map(|event| {
            json!({
                "action": event.action.as_str(),
                "detail": event.detail,
                "by_someone_else": event.actor_id.is_some_and(|actor| actor != event.user_id),
                "ip": event.client.ip.map(|ip| ip.to_string()),
                "user_agent": event.client.user_agent,
                "occurred_at": timestamp(event.occurred_at),
            })
        }));
        match page.next {
            Some(next) => after = Some(next),
            None => return Ok(serde_json::Value::Array(events)),
        }
    }
}

fn phone_taken() -> AppError {
    AppError::invalid(
        "phone",
        &ValidationError::new("phone_taken", Message::new("validation-phone-taken")),
    )
}
