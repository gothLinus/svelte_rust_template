use std::sync::Arc;

use domain::{
    audit::{AuditAction, AuditRepository, AuthMethod},
    clock::Clock,
    database::{Database, Transaction},
    i18n::Message,
    mfa::{MfaRepository, TOTP_SECRET_BYTES},
    passkey::PasskeyRepository,
    secret::Secret,
    security::Crypto,
    session::ClientInfo,
    user::{User, UserId, UserRepository},
};

use crate::{
    Adapters, Context,
    actor::Actor,
    auth::signin::{SignedIn, second_factors},
    codes,
    dto::MfaChallengeDto,
    error::AppError,
    mail,
    mfa::{
        challenge,
        dto::{CodeRequest, RecoveryCodesDto, SecondFactorAddedDto, TotpSetupDto},
        totp,
    },
};

/// Authenticator app enrolment, recovery codes and completing the second sign-in step.
///
/// Enrolling and removing factors need a recent sign-in
/// (`Actor::require_recent_authentication`); completing a step is authorised by the `mfa` token
/// alone.
pub struct MfaService<A: Adapters> {
    ctx: Arc<Context<A>>,
}

impl<A: Adapters> MfaService<A> {
    pub fn new(ctx: Arc<Context<A>>) -> Self {
        Self { ctx }
    }

    /// Starts setting up an authenticator app: a new secret, not used until
    /// [`MfaService::confirm_totp`] sees a code from it.
    ///
    /// # Errors
    ///
    /// Needs a verified address and a recent sign-in. `409 totp_enabled` when an app is confirmed
    /// already; an unconfirmed setup is replaced.
    pub async fn start_totp(&self, actor: &Actor) -> Result<TotpSetupDto, AppError> {
        actor.require_verified_email()?;
        actor.require_recent_authentication()?;
        let user = self.user(actor).await?;
        let secret = self.ctx.crypto.random_bytes(TOTP_SECRET_BYTES)?;
        let sealed = self.ctx.crypto.seal(&secret, &totp::context(user.id()))?;

        if !self
            .ctx
            .db
            .connection()
            .await?
            .start_totp_setup(user.id(), &sealed, self.ctx.clock.now())
            .await?
        {
            return Err(AppError::conflict(
                "totp_enabled",
                Message::new("conflict-totp-enabled"),
            ));
        }

        let secret = totp::base32(&secret);
        let uri = totp::otpauth_uri(&self.ctx.settings.app_name, user.email().as_str(), &secret);
        Ok(TotpSetupDto { secret, uri })
    }

    /// Finishes setting up the app with a first code from it. Returns recovery codes if this turned
    /// two-step sign-in on. The code's time step is recorded, so it cannot be replayed at the next
    /// sign-in.
    pub async fn confirm_totp(
        &self,
        actor: &Actor,
        request: CodeRequest,
    ) -> Result<SecondFactorAddedDto, AppError> {
        actor.require_verified_email()?;
        actor.require_recent_authentication()?;
        let user = self.user(actor).await?;
        let now = self.ctx.clock.now();
        let mut tx = self.ctx.db.transaction().await?;
        let pending = tx
            .find_totp(user.id())
            .await?
            .filter(|totp| !totp.is_confirmed())
            .ok_or_else(|| {
                AppError::conflict(
                    "totp_not_started",
                    Message::new("conflict-totp-not-started"),
                )
            })?;
        let secret = self
            .ctx
            .crypto
            .open(&pending.sealed_secret, &totp::context(user.id()))?;
        let step = totp::matching_step(&self.ctx.crypto, &secret, request.code.0.expose(), now)
            .ok_or_else(AppError::invalid_code)?;

        let first_factor = second_factors(&mut tx, user.id()).await?.is_empty();
        if !tx.confirm_totp(user.id(), now, step).await? {
            return Err(AppError::invalid_code());
        }
        let recovery_codes = if first_factor {
            Some(challenge::new_recovery_codes(&self.ctx, &mut tx, user.id()).await?)
        } else {
            None
        };
        tx.record_audit_event(&self.ctx.actor_event(actor, AuditAction::TotpAdded))
            .await?;
        tx.commit().await?;

        self.notify(&user, Message::new("notice-totp-added")).await;
        tracing::info!(user_id = %user.id(), "authenticator app added");
        Ok(SecondFactorAddedDto { recovery_codes })
    }

    /// Removes the authenticator app. Asks for a current code from it, so a stolen session alone
    /// cannot turn it off.
    pub async fn remove_totp(&self, actor: &Actor, request: CodeRequest) -> Result<(), AppError> {
        let user = self.user(actor).await?;
        let now = self.ctx.clock.now();
        let mut tx = self.ctx.db.transaction().await?;
        let current = tx
            .find_totp(user.id())
            .await?
            .filter(domain::mfa::TotpCredential::is_confirmed)
            .ok_or(AppError::NotFound)?;
        let secret = self
            .ctx
            .crypto
            .open(&current.sealed_secret, &totp::context(user.id()))?;
        let step = totp::matching_step(&self.ctx.crypto, &secret, request.code.0.expose(), now)
            .ok_or_else(AppError::invalid_code)?;
        if !tx.use_totp_step(user.id(), step).await? {
            return Err(AppError::invalid_code());
        }

        tx.delete_totp(user.id()).await?;
        if tx.count_user_passkeys(user.id()).await? == 0 {
            tx.replace_recovery_codes(user.id(), &[]).await?;
        }
        tx.record_audit_event(&self.ctx.actor_event(actor, AuditAction::TotpRemoved))
            .await?;
        tx.commit().await?;

        self.notify(&user, Message::new("notice-totp-removed"))
            .await;
        tracing::info!(user_id = %user.id(), "authenticator app removed");
        Ok(())
    }

    /// Replaces the recovery codes, such as after using some or losing the list. The old ones stop
    /// working; the new ones are returned once and stored only as digests.
    ///
    /// # Errors
    ///
    /// `409 mfa_disabled` when the user has no second factor.
    pub async fn regenerate_recovery_codes(
        &self,
        actor: &Actor,
    ) -> Result<RecoveryCodesDto, AppError> {
        actor.require_recent_authentication()?;
        let mut tx = self.ctx.db.transaction().await?;
        if second_factors(&mut tx, actor.user_id).await?.is_empty() {
            return Err(AppError::conflict(
                "mfa_disabled",
                Message::new("conflict-mfa-disabled"),
            ));
        }
        let codes = challenge::new_recovery_codes(&self.ctx, &mut tx, actor.user_id).await?;
        tx.record_audit_event(
            &self
                .ctx
                .actor_event(actor, AuditAction::RecoveryCodesRegenerated),
        )
        .await?;
        tx.commit().await?;
        tracing::info!(user_id = %actor.user_id, "recovery codes regenerated");
        Ok(RecoveryCodesDto { codes })
    }

    pub async fn pending(&self, token: &Secret) -> Result<MfaChallengeDto, AppError> {
        let challenge = challenge::load(&self.ctx, token).await?;
        let mut conn = self.ctx.db.connection().await?;
        let methods = second_factors(&mut conn, challenge.user_id).await?;
        Ok(MfaChallengeDto { methods })
    }

    /// The user behind the pending sign-in, for rate limiting the second step per account rather
    /// than per attempt.
    pub async fn pending_user(&self, token: &Secret) -> Result<UserId, AppError> {
        Ok(challenge::load(&self.ctx, token).await?.user_id)
    }

    /// Finishes a sign-in with a code from the authenticator app.
    ///
    /// # Errors
    ///
    /// `mfa_expired` when the token is unknown, expired, used or out of attempts;
    /// [`AppError::invalid_code`] for a wrong code, one already used, or a user without an app. The
    /// attempt is counted before the code is checked.
    pub async fn complete_with_totp(
        &self,
        token: &Secret,
        request: CodeRequest,
        client: ClientInfo,
        previous: Option<&Secret>,
    ) -> Result<SignedIn, AppError> {
        let challenge = challenge::reserve(&self.ctx, token).await?;
        let now = self.ctx.clock.now();
        let mut conn = self.ctx.db.connection().await?;
        let totp = conn
            .find_totp(challenge.user_id)
            .await?
            .filter(domain::mfa::TotpCredential::is_confirmed);

        let step = match &totp {
            Some(totp) => {
                let secret = self
                    .ctx
                    .crypto
                    .open(&totp.sealed_secret, &totp::context(challenge.user_id))?;
                totp::matching_step(&self.ctx.crypto, &secret, request.code.0.expose(), now)
            }
            None => None,
        };
        let accepted = match step {
            Some(step) => conn.use_totp_step(challenge.user_id, step).await?,
            None => false,
        };
        drop(conn);
        if !accepted {
            challenge::record_failure(&self.ctx, &challenge, &client, AuthMethod::Totp).await?;
            return Err(AppError::invalid_code());
        }
        challenge::finish(&self.ctx, &challenge, client, previous, AuthMethod::Totp).await
    }

    /// Finishes a sign-in with a recovery code, which is used up. Errors as
    /// [`MfaService::complete_with_totp`].
    pub async fn complete_with_recovery_code(
        &self,
        token: &Secret,
        request: CodeRequest,
        client: ClientInfo,
        previous: Option<&Secret>,
    ) -> Result<SignedIn, AppError> {
        let challenge = challenge::reserve(&self.ctx, token).await?;
        let guess = codes::normalize(request.code.0.expose()).to_lowercase();
        let used = guess.len() == 10
            && self
                .consume_recovery_code(challenge.user_id, &guess)
                .await?;
        if !used {
            challenge::record_failure(&self.ctx, &challenge, &client, AuthMethod::RecoveryCode)
                .await?;
            return Err(AppError::invalid_code());
        }

        let signed_in = challenge::finish(
            &self.ctx,
            &challenge,
            client,
            previous,
            AuthMethod::RecoveryCode,
        )
        .await?;
        tracing::info!(user_id = %challenge.user_id, "signed in with a recovery code");
        Ok(signed_in)
    }

    /// Uses up `code` if it is one of the user's recovery codes, made under the current key or,
    /// during a key rotation, the previous one.
    async fn consume_recovery_code(&self, user: UserId, code: &str) -> Result<bool, AppError> {
        let mut conn = self.ctx.db.connection().await?;
        if conn
            .consume_recovery_code(user, &codes::digest(&self.ctx, user, code))
            .await?
        {
            return Ok(true);
        }
        match codes::previous_digest(&self.ctx, user, code) {
            Some(previous) => Ok(conn.consume_recovery_code(user, &previous).await?),
            None => Ok(false),
        }
    }

    async fn user(&self, actor: &Actor) -> Result<User, AppError> {
        self.ctx
            .db
            .connection()
            .await?
            .find_user(actor.user_id)
            .await?
            .ok_or(AppError::Unauthenticated)
    }

    async fn notify(&self, user: &User, what: Message) {
        mail::notify(&self.ctx, user, what).await;
    }
}
