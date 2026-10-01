//! Step-up re-authentication: proving again who is behind a session before a sensitive change.
//!
//! Signing in counts, for [`REAUTH_WINDOW`]. After that, adding or removing a sign-in method,
//! changing the address or phone number, regenerating recovery codes or deleting the account
//! answer `403 reauth_required` until the session re-authenticates with any method the user has:
//! the password, an authenticator code, a passkey (see
//! [`PasskeyService::reauthenticate`](crate::passkeys::PasskeyService::reauthenticate)), or a
//! code mailed to the account's address. A stolen session alone is then not enough to lock the
//! owner out.

use std::sync::Arc;

use domain::{
    audit::{AuditAction, AuditRepository, AuthMethod},
    clock::Clock,
    database::{Database, Transaction},
    i18n::Message,
    mfa::{MfaRepository, TotpCredential},
    one_time_code::{CodeChannel, CodePurpose},
    passkey::PasskeyRepository,
    security::{Crypto, PasswordHasher},
    session::{REAUTH_WINDOW, SessionRepository},
    user::{User, UserRepository},
};

use crate::{
    Adapters, Context,
    actor::Actor,
    auth::dto::{ReauthMethod, ReauthMethodsDto, ReauthenticateRequest},
    codes,
    error::AppError,
    mail,
    mfa::totp,
    passwordless::group,
};

/// Re-authenticates a session so it may make sensitive changes for [`REAUTH_WINDOW`]. The
/// services of sensitive changes call `Actor::require_recent_authentication`, answering
/// `403 reauth_required`; this service is how a client satisfies that check.
pub struct ReauthService<A: Adapters> {
    ctx: Arc<Context<A>>,
}

impl<A: Adapters> ReauthService<A> {
    pub fn new(ctx: Arc<Context<A>>) -> Self {
        Self { ctx }
    }

    pub async fn methods(&self, actor: &Actor) -> Result<ReauthMethodsDto, AppError> {
        let mut conn = self.ctx.db.connection().await?;
        let user = conn
            .find_user(actor.user_id)
            .await?
            .ok_or(AppError::Unauthenticated)?;
        let mut methods = Vec::new();
        if conn.count_user_passkeys(user.id()).await? > 0 {
            methods.push(ReauthMethod::Passkey);
        }
        if user.has_password() {
            methods.push(ReauthMethod::Password);
        }
        if conn
            .find_totp(user.id())
            .await?
            .is_some_and(|totp| totp.is_confirmed())
        {
            methods.push(ReauthMethod::Totp);
        }
        methods.push(ReauthMethod::EmailCode);
        Ok(ReauthMethodsDto { methods })
    }

    pub async fn send_email_code(&self, actor: &Actor) -> Result<(), AppError> {
        let mut conn = self.ctx.db.connection().await?;
        let user = conn
            .find_user(actor.user_id)
            .await?
            .ok_or(AppError::Unauthenticated)?;
        let code = codes::issue(
            &self.ctx,
            &mut conn,
            user.id(),
            CodePurpose::Reauthentication,
            CodeChannel::Email,
            None,
        )
        .await?;
        drop(conn);
        mail::send(
            &*self.ctx.mailer,
            mail::reauthentication_code(
                &self.ctx.voice_for(&user),
                user.email().clone(),
                &group(code.expose()),
            ),
        )
        .await;
        Ok(())
    }

    /// Proves the actor again with a password, an authenticator code or an emailed code, and marks
    /// the session re-authenticated.
    ///
    /// # Errors
    ///
    /// A field error on `password` for a wrong password (a user without one never matches),
    /// [`AppError::invalid_code`] for a wrong, expired or already used code. An authenticator code
    /// works once, as at sign-in, and emailed codes count wrong guesses against a limit.
    pub async fn reauthenticate(
        &self,
        actor: &Actor,
        request: ReauthenticateRequest,
    ) -> Result<(), AppError> {
        let mut conn = self.ctx.db.connection().await?;
        let user = conn
            .find_user(actor.user_id)
            .await?
            .ok_or(AppError::Unauthenticated)?;
        drop(conn);
        let method = match &request {
            ReauthenticateRequest::Password { .. } => AuthMethod::Password,
            ReauthenticateRequest::Totp { .. } => AuthMethod::Totp,
            ReauthenticateRequest::EmailCode { .. } => AuthMethod::EmailCode,
        };
        match request {
            ReauthenticateRequest::Password { password } => {
                self.check_password(&user, &password.0).await?;
            }
            ReauthenticateRequest::Totp { code } => self.check_totp(&user, code.0.expose()).await?,
            ReauthenticateRequest::EmailCode { code } => {
                let mut conn = self.ctx.db.connection().await?;
                codes::check(
                    &self.ctx,
                    &mut conn,
                    user.id(),
                    CodePurpose::Reauthentication,
                    &[CodeChannel::Email],
                    code.0.expose(),
                )
                .await
                .map_err(|_| AppError::invalid_code())?;
            }
        }
        mark(&self.ctx, actor, method).await
    }

    /// `Ok` if `password` is the user's. A user without a password never matches.
    pub(crate) async fn check_password(
        &self,
        user: &User,
        password: &domain::secret::Secret,
    ) -> Result<(), AppError> {
        let matches = self
            .ctx
            .hasher
            .verify(password, user.password_hash())
            .await?;
        if matches && user.has_password() {
            Ok(())
        } else {
            Err(wrong_password())
        }
    }

    async fn check_totp(&self, user: &User, code: &str) -> Result<(), AppError> {
        let mut conn = self.ctx.db.connection().await?;
        let Some(credential) = conn
            .find_totp(user.id())
            .await?
            .filter(TotpCredential::is_confirmed)
        else {
            return Err(AppError::invalid_code());
        };
        let secret = self
            .ctx
            .crypto
            .open(&credential.sealed_secret, &totp::context(user.id()))?;
        let step = totp::matching_step(&self.ctx.crypto, &secret, code, self.ctx.clock.now())
            .ok_or_else(AppError::invalid_code)?;
        // A code works once, here as at sign-in.
        if conn.use_totp_step(user.id(), step).await? {
            Ok(())
        } else {
            Err(AppError::invalid_code())
        }
    }
}

/// Marks the actor's session re-authenticated by `method`, and records it.
pub(crate) async fn mark<A: Adapters>(
    ctx: &Context<A>,
    actor: &Actor,
    method: AuthMethod<'_>,
) -> Result<(), AppError> {
    let now = ctx.clock.now();
    let mut tx = ctx.db.transaction().await?;
    tx.mark_session_reauthenticated(actor.session_id, now)
        .await?;
    tx.record_audit_event(
        &ctx.actor_event(actor, AuditAction::Reauthenticated)
            .detail(method.detail()),
    )
    .await?;
    tx.commit().await?;
    tracing::info!(
        user_id = %actor.user_id,
        window_minutes = REAUTH_WINDOW.whole_minutes(),
        "session re-authenticated"
    );
    Ok(())
}

/// A wrong password, reported on its field so the form can show it there. Wrong codes are
/// [`AppError::invalid_code`], like everywhere else.
fn wrong_password() -> AppError {
    AppError::invalid(
        "password",
        &domain::error::ValidationError::new(
            "invalid_credential",
            Message::new("validation-invalid-credential"),
        ),
    )
}
