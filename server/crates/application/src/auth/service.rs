use std::sync::Arc;

use crate::{
    Adapters, Context, Store,
    actor::Actor,
    auth::{
        access,
        dto::{
            AuthMethodsDto, CancelEmailChangeRequest, ConfirmEmailRequest, ForgotPasswordRequest,
            LoginRequest, PasswordReset, RegisterRequest, Registration, ResetPasswordRequest,
            VerificationPending, VerifyEmailRequest,
        },
        signin::{self, LoginOutcome, SignedIn},
    },
    error::AppError,
    mail,
    oauth::dto::ProviderDto,
    passwordless::dto::TextChannel,
    tokens,
};
use domain::{
    audit::{AuditAction, AuditRepository, AuthMethod},
    clock::Clock,
    database::{Database, Transaction},
    i18n::{Locale, Message},
    rbac::RbacRepository,
    secret::Secret,
    security::{PasswordHasher, TokenGenerator},
    session::{AuthenticatedSession, ClientInfo, Session, SessionId, SessionRepository},
    user::{
        EMAIL_UNIQUE_CONSTRAINT, Email, LoginIdentifier, NewUser, USERNAME_UNIQUE_CONSTRAINT, User,
        UserId, UserRepository,
    },
    user_token::{TokenPurpose, UserTokenRepository},
};

#[derive(Debug)]
pub enum Registered {
    SignedIn(Box<SignedIn>),
    /// Sign-in requires a verified address; a verification link was sent. `browser` marks the
    /// registering browser (the `registration` cookie), so opening the link there keeps the
    /// password. For an already registered address it is a decoy that matches no account, so the
    /// response looks the same.
    VerificationPending {
        pending: VerificationPending,
        browser: Secret,
    },
}

/// The account a sign-in names, from [`AuthService::login_account`]. Opaque, so a sign-in cannot
/// be pointed at an account the identifier did not name.
#[derive(Debug)]
pub struct LoginAccount(Option<User>);

impl LoginAccount {
    pub fn id(&self) -> Option<UserId> {
        self.0.as_ref().map(User::id)
    }
}

/// What the browser that opens an emailed link carries: its session and the mark left by
/// registering, either of which shows it is the browser that registered the account.
#[derive(Debug, Clone, Copy, Default)]
pub struct Browser<'a> {
    pub session: Option<&'a Secret>,
    pub registration: Option<&'a Secret>,
}

#[derive(Debug)]
pub struct Authenticated {
    pub actor: Actor,
    pub user: User,
    pub session: Session,
    /// A replacement token, set when the user's privileges changed since the old one was issued.
    /// The caller must send it to the client; the old token no longer works.
    pub rotated_token: Option<Secret>,
}

/// Registration, sign-in and sign-out, session lookup, email verification, password reset and
/// email change. The other sign-in methods finish through the same `signin::complete_first_step`;
/// see the [`auth`](crate::auth) module.
pub struct AuthService<A: Adapters> {
    ctx: Arc<Context<A>>,
}

impl<A: Adapters> AuthService<A> {
    pub fn new(ctx: Arc<Context<A>>) -> Self {
        Self { ctx }
    }

    /// Creates the account with the default role, mails a verification link and, unless
    /// verification is required first, signs the new user in.
    ///
    /// A taken address is `409 email_taken` when sign-in needs no verified address, a deliberate
    /// trade-off: the user learns it at once and can sign in instead. With verification required it
    /// is answered like a new address, and its owner gets a "you already have an account" mail.
    ///
    /// `locale` is the language the request asked for; mail to the account is written in it until
    /// the user picks another.
    pub async fn register(
        &self,
        request: RegisterRequest,
        client: ClientInfo,
        locale: Option<Locale>,
    ) -> Result<Registered, AppError> {
        let registration = Registration::try_from(request)?;
        let password_hash = self
            .ctx
            .hasher
            .hash(registration.password.as_secret())
            .await?;
        let now = self.ctx.clock.now();

        let mut tx = self.ctx.db.transaction().await?;
        let new_user = NewUser {
            id: UserId::generate_at(now),
            email: registration.email,
            username: registration.username,
            password_hash: Some(password_hash),
            email_verified_at: None,
            locale,
        };
        let user = match tx.create_user(&new_user).await {
            Ok(user) => user,
            // With verification required, the answer is the same as for a new address, so
            // registering does not reveal which addresses have an account. Both paths hashed the
            // password first, which dwarfs the few statements a new account adds.
            Err(err)
                if err.is_unique_violation(EMAIL_UNIQUE_CONSTRAINT)
                    && self.ctx.settings.require_email_verification =>
            {
                drop(tx);
                self.send_already_registered(&new_user.email).await?;
                return Ok(Registered::VerificationPending {
                    pending: VerificationPending {
                        email: new_user.email.to_string(),
                    },
                    browser: tokens::decoy_registration_mark(&self.ctx, now)?,
                });
            }
            Err(err) if err.is_unique_violation(EMAIL_UNIQUE_CONSTRAINT) => {
                return Err(AppError::conflict(
                    "email_taken",
                    Message::new("conflict-email-taken"),
                ));
            }
            Err(err) if err.is_unique_violation(USERNAME_UNIQUE_CONSTRAINT) => {
                return Err(crate::error::username_taken());
            }
            Err(err) => return Err(err.into()),
        };
        tx.grant_role(user.id(), &domain::rbac::RoleName::USER)
            .await?;
        tx.record_audit_event(&self.ctx.event(user.id(), AuditAction::Registered, &client))
            .await?;
        let verification = tokens::issue(
            &self.ctx,
            &mut tx,
            user.id(),
            TokenPurpose::EmailVerification,
            now,
        )
        .await?;

        let outcome = if self.ctx.settings.require_email_verification {
            tx.commit().await?;
            Registered::VerificationPending {
                pending: VerificationPending {
                    email: user.email().to_string(),
                },
                browser: tokens::registration_mark(&self.ctx, user.id(), now),
            }
        } else {
            let signed_in = signin::start_session(
                &self.ctx,
                &mut tx,
                &user,
                client,
                None,
                AuthMethod::Registration,
            )
            .await?;
            tx.commit().await?;
            Registered::SignedIn(Box::new(signed_in))
        };

        self.send_verification(&user, &verification).await;
        tracing::info!(user_id = %user.id(), "account registered");
        Ok(outcome)
    }

    pub fn methods(&self) -> AuthMethodsDto {
        AuthMethodsDto {
            app_name: self.ctx.settings.app_name.clone(),
            providers: self
                .ctx
                .identity_providers
                .providers()
                .iter()
                .map(|provider| ProviderDto {
                    id: provider.id.clone(),
                    name: provider.name.clone(),
                })
                .collect(),
            text_channels: self
                .ctx
                .texts
                .channels()
                .iter()
                .filter_map(|channel| TextChannel::from_code_channel(*channel))
                .collect(),
            lifetimes: self.ctx.settings.tokens.into(),
        }
    }

    /// Checks the credentials and starts a session, or the second step if the user has one.
    ///
    /// The identifier is an email address, a username or a verified phone number. Unknown
    /// identifiers and wrong passwords get the same error and take the same time: the password is
    /// verified against a decoy hash when there is no account or it has no password. `previous` is
    /// the session token the browser already had, if any; it is revoked, so signing in always
    /// yields a new token.
    ///
    /// # Errors
    ///
    /// [`AppError::InvalidCredentials`] for an unknown identifier or a wrong password. A disabled
    /// account or (when required) an unverified address is reported only after the password
    /// matched, so those errors reveal nothing to someone guessing.
    pub async fn login(
        &self,
        request: LoginRequest,
        client: ClientInfo,
        previous: Option<&Secret>,
    ) -> Result<LoginOutcome, AppError> {
        let account = self.login_account(&request.identifier).await?;
        self.login_as(account, &request.password.0, client, previous)
            .await
    }

    /// [`AuthService::login`] for an account already looked up with [`AuthService::login_account`],
    /// which the caller needed for rate limiting.
    pub async fn login_as(
        &self,
        account: LoginAccount,
        password: &Secret,
        client: ClientInfo,
        previous: Option<&Secret>,
    ) -> Result<LoginOutcome, AppError> {
        let user = account.0;
        let matches = self
            .ctx
            .hasher
            .verify(password, user.as_ref().and_then(User::password_hash))
            .await?;
        let user = match user {
            Some(user) if matches => user,
            Some(user) => {
                self.record_failure(&user, &client).await?;
                return Err(AppError::InvalidCredentials);
            }
            None => return Err(AppError::InvalidCredentials),
        };
        if user
            .password_hash()
            .is_some_and(|hash| self.ctx.hasher.needs_rehash(hash))
        {
            self.rehash(&user, password).await?;
        }

        signin::complete_first_step(&self.ctx, &user, client, previous, AuthMethod::Password).await
    }

    /// Records a wrong password for an existing account. Unknown identifiers are not recorded:
    /// there is no account to file them under.
    async fn record_failure(&self, user: &User, client: &ClientInfo) -> Result<(), AppError> {
        self.ctx
            .db
            .connection()
            .await?
            .record_audit_event(
                &self
                    .ctx
                    .event(user.id(), AuditAction::SignInFailed, client)
                    .detail(AuthMethod::Password.detail()),
            )
            .await?;
        Ok(())
    }

    async fn rehash(&self, user: &User, password: &Secret) -> Result<(), AppError> {
        let hash = self.ctx.hasher.hash(password).await?;
        self.ctx
            .db
            .connection()
            .await?
            .set_user_password(user.id(), Some(&hash))
            .await?;
        tracing::info!(user_id = %user.id(), "password rehashed with the current parameters");
        Ok(())
    }

    /// The account a sign-in identifier (an email address, username or verified phone number)
    /// names, if any, looked up once for both rate limiting and [`AuthService::login_as`]. Charging
    /// the account rather than the identifier gives its address, username and number one shared
    /// budget.
    pub async fn login_account(&self, identifier: &str) -> Result<LoginAccount, AppError> {
        let Ok(identifier) = LoginIdentifier::parse(identifier) else {
            return Ok(LoginAccount(None));
        };
        let mut conn = self.ctx.db.connection().await?;
        Ok(LoginAccount(
            find_by_identifier(&mut conn, &identifier).await?,
        ))
    }

    /// A mark for a browser `user` just signed in on (a cookie); see
    /// [`AuthService::is_known_device`].
    pub fn known_device_mark(&self, user: UserId) -> Secret {
        tokens::device_mark(&self.ctx, user, self.ctx.clock.now())
    }

    /// Whether `mark` shows the browser is one `user` signed in on before. Password sign-ins from
    /// it skip the account-wide ceiling on failures, so attempts from elsewhere cannot lock the
    /// owner out.
    pub fn is_known_device(&self, user: UserId, mark: &Secret) -> bool {
        tokens::is_device_mark(&self.ctx, user, mark, self.ctx.clock.now())
    }

    /// Resolves a session token to its user. Unknown, expired and revoked tokens resolve to `None`,
    /// and so do disabled accounts and, when verification is required, unverified ones. Expired
    /// sessions are deleted on the way. Slides the idle timeout and rotates the token if the user's
    /// privileges changed ([`Authenticated::rotated_token`]).
    ///
    /// The one lookup also loads the user's permissions, so a demoted user loses access on the next
    /// request. `client` is the request's, carried by the [`Actor`] into the audit log.
    pub async fn authenticate(
        &self,
        token: &Secret,
        client: ClientInfo,
    ) -> Result<Option<Authenticated>, AppError> {
        if token.is_empty() || !token.is_within_limit() {
            return Ok(None);
        }
        let digest = self.ctx.tokens.digest(token);
        let mut conn = self.ctx.db.connection().await?;

        let Some(AuthenticatedSession {
            mut session,
            user,
            permissions,
        }) = conn.find_session_by_token(&digest).await?
        else {
            return Ok(None);
        };

        let now = self.ctx.clock.now();
        let policy = &self.ctx.settings.sessions;
        if !session.is_active(now, policy) || user.is_disabled() {
            conn.delete_session(session.id()).await?;
            return Ok(None);
        }
        if self.ctx.settings.require_email_verification && !user.is_email_verified() {
            return Ok(None);
        }

        let rotated_token = if session.rotation_pending() {
            let token = self.ctx.tokens.generate()?;
            session.rotate(self.ctx.tokens.digest(&token), now);
            // Losing the race to a concurrent request is fine: it hands out the new token.
            conn.rotate_session(&session, &digest)
                .await?
                .then_some(token)
        } else {
            if session.needs_touch(now, policy) {
                session.touch(now);
                conn.touch_session(&session).await?;
            }
            None
        };

        let actor = Actor {
            user_id: user.id(),
            session_id: session.id(),
            permissions,
            email_verified: user.is_email_verified(),
            recently_authenticated: session.is_recently_authenticated(now),
            client,
        };
        Ok(Some(Authenticated {
            actor,
            user,
            session,
            rotated_token,
        }))
    }

    /// Ends the session behind `token`. Succeeds if there is none, so signing out twice is
    /// harmless.
    pub async fn logout(&self, token: &Secret) -> Result<(), AppError> {
        self.ctx
            .db
            .connection()
            .await?
            .delete_session_by_token(&self.ctx.tokens.digest(token))
            .await?;
        Ok(())
    }

    /// Ends every session of the actor, the current one included, and every pending link, code and
    /// second step. Returns how many sessions.
    pub async fn logout_everywhere(&self, actor: &Actor) -> Result<u64, AppError> {
        let mut tx = self.ctx.db.transaction().await?;
        let revoked = access::revoke_all(&mut tx, actor.user_id, None).await?;
        tx.record_audit_event(
            &self
                .ctx
                .actor_event(actor, AuditAction::SignedOutEverywhere),
        )
        .await?;
        tx.commit().await?;
        tracing::info!(user_id = %actor.user_id, revoked, "signed out everywhere");
        Ok(revoked)
    }

    /// Marks the address behind a verification link as verified. The link works once.
    ///
    /// If this is the first proof that the address's owner controls the account, every other way in
    /// is evicted (see `access::prove_email_ownership`). Opened in the browser that registered the
    /// account, the password chosen there and that session stay; anywhere else the password is
    /// removed too, since a stranger may have chosen it, and the owner is mailed a link to choose
    /// their own.
    ///
    /// # Errors
    ///
    /// [`AppError::InvalidToken`] for a link that is unknown, expired or already used.
    pub async fn verify_email(
        &self,
        request: VerifyEmailRequest,
        browser: Browser<'_>,
        client: ClientInfo,
    ) -> Result<(), AppError> {
        let token = request.token.0;
        if token.is_empty() || !token.is_within_limit() {
            return Err(AppError::InvalidToken);
        }
        let now = self.ctx.clock.now();

        let mut tx = self.ctx.db.transaction().await?;
        let user_id = tx
            .consume_user_token(
                &self.ctx.tokens.digest(&token),
                TokenPurpose::EmailVerification,
                now,
            )
            .await?
            .ok_or(AppError::InvalidToken)?
            .user_id;
        let keep = self.own_session(&mut tx, browser.session, user_id).await?;
        let registrant = keep.is_some()
            || browser.registration.is_some_and(|mark| {
                mark.is_within_limit()
                    && tokens::is_registration_mark(&self.ctx, user_id, mark, now)
            });
        let prover = if registrant {
            access::Prover::Registrant { keep }
        } else {
            access::Prover::Owner
        };
        let outcome = access::prove_email_ownership(&mut tx, user_id, now, prover, &client).await?;
        tx.commit().await?;

        if outcome.password_removed {
            access::offer_password(&self.ctx, &outcome.user).await;
        }
        tracing::info!(%user_id, "email verified");
        Ok(())
    }

    pub async fn resend_verification(&self, actor: &Actor) -> Result<(), AppError> {
        let mut conn = self.ctx.db.connection().await?;
        let user = conn
            .find_user(actor.user_id)
            .await?
            .ok_or(AppError::Unauthenticated)?;
        if user.is_email_verified() {
            return Err(AppError::conflict(
                "already_verified",
                Message::new("conflict-already-verified"),
            ));
        }

        let token = tokens::issue(
            &self.ctx,
            &mut conn,
            user.id(),
            TokenPurpose::EmailVerification,
            self.ctx.clock.now(),
        )
        .await?;
        drop(conn);
        self.send_verification(&user, &token).await;
        Ok(())
    }

    /// Mails a reset link if an enabled account uses the address. A new link replaces any earlier
    /// one.
    ///
    /// Succeeds either way, so the response does not reveal which addresses have an account. Mail
    /// is queued rather than sent inline, so both paths take about as long.
    pub async fn request_password_reset(
        &self,
        request: ForgotPasswordRequest,
    ) -> Result<(), AppError> {
        let email = Email::parse(&request.email).map_err(|err| AppError::invalid("email", &err))?;

        let mut conn = self.ctx.db.connection().await?;
        let Some(user) = conn
            .find_user_by_email(&email)
            .await?
            .filter(|user| !user.is_disabled())
        else {
            return Ok(());
        };

        drop(conn);
        self.send_password_reset(&user).await
    }

    /// Mails `user` a link to choose a new password. Changing the password from the settings goes
    /// through this link too, so it needs access to the mailbox.
    pub(crate) async fn send_password_reset(&self, user: &User) -> Result<(), AppError> {
        let mut conn = self.ctx.db.connection().await?;
        let token = tokens::issue(
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
            mail::password_reset(&self.ctx.voice_for(user), user.email().clone(), &link, ttl),
        )
        .await;
        tracing::info!(user_id = %user.id(), "password reset link sent");
        Ok(())
    }

    /// Sets a new password with a token from a reset link, and signs out every session: whoever had
    /// access before must not keep it. Opening the link also proves the user owns the address, so
    /// it counts as verified (see `access::prove_email_ownership` for what the first proof evicts).
    /// The link works once, and every pending link and code of the account is revoked with the
    /// sessions.
    ///
    /// # Errors
    ///
    /// [`AppError::InvalidToken`] for a link that is unknown, expired or already used; a validation
    /// error for a password that does not meet the rules.
    pub async fn reset_password(
        &self,
        request: ResetPasswordRequest,
        client: ClientInfo,
    ) -> Result<(), AppError> {
        let reset = PasswordReset::try_from(request)?;
        if !reset.token.is_within_limit() {
            return Err(AppError::InvalidToken);
        }
        let digest = self.ctx.tokens.digest(&reset.token);
        let now = self.ctx.clock.now();

        // Hashed only once the token checked out, so bogus tokens cost no CPU, and before the
        // transaction opens, so no pooled connection waits on Argon2. The token is consumed (and
        // checked again) below.
        self.ctx
            .db
            .connection()
            .await?
            .user_token_exists(&digest, TokenPurpose::PasswordReset, now)
            .await?
            .then_some(())
            .ok_or(AppError::InvalidToken)?;
        let hash = self.ctx.hasher.hash(reset.password.as_secret()).await?;

        let mut tx = self.ctx.db.transaction().await?;
        let user_id = tx
            .consume_user_token(&digest, TokenPurpose::PasswordReset, now)
            .await?
            .ok_or(AppError::InvalidToken)?
            .user_id;
        // The first proof evicts whoever got in before, their password included, so the new one is
        // set after it.
        let user =
            access::prove_email_ownership(&mut tx, user_id, now, access::Prover::Owner, &client)
                .await?
                .user;
        if !tx.set_user_password(user_id, Some(&hash)).await? {
            return Err(AppError::InvalidToken);
        }
        // Whoever had access before must not keep it: sessions, but also pending links, codes and
        // second steps.
        access::revoke_all(&mut tx, user_id, None).await?;
        tx.record_audit_event(&self.ctx.event(user_id, AuditAction::PasswordReset, &client))
            .await?;
        tx.commit().await?;

        let forgot = self.ctx.settings.links.forgot_password();
        mail::send(
            &*self.ctx.mailer,
            mail::password_changed(&self.ctx.voice_for(&user), user.email().clone(), &forgot),
        )
        .await;
        tracing::info!(%user_id, "password reset");
        Ok(())
    }

    /// Makes the address behind an email-change link the account's address. Opening the link proves
    /// the user owns it, so it counts as verified. Every session but the browser's own (`current`,
    /// if it belongs to the account) is signed out: whoever else was signed in should not follow
    /// the account to its new address.
    pub async fn confirm_email_change(
        &self,
        request: ConfirmEmailRequest,
        current: Option<&Secret>,
        client: ClientInfo,
    ) -> Result<(), AppError> {
        let token = request.token.0;
        if token.is_empty() || !token.is_within_limit() {
            return Err(AppError::InvalidToken);
        }
        let now = self.ctx.clock.now();

        let mut tx = self.ctx.db.transaction().await?;
        let consumed = tx
            .consume_user_token(
                &self.ctx.tokens.digest(&token),
                TokenPurpose::EmailChange,
                now,
            )
            .await?
            .ok_or(AppError::InvalidToken)?;
        let new_email = consumed.email.ok_or(AppError::InvalidToken)?;
        let previous = tx
            .find_user(consumed.user_id)
            .await?
            .ok_or(AppError::InvalidToken)?;
        let user = match tx
            .change_user_email(consumed.user_id, &new_email, now)
            .await
        {
            Ok(user) => user.ok_or(AppError::InvalidToken)?,
            Err(err) if err.is_unique_violation(EMAIL_UNIQUE_CONSTRAINT) => {
                return Err(AppError::conflict(
                    "email_taken",
                    Message::new("conflict-email-taken"),
                ));
            }
            Err(err) => return Err(err.into()),
        };
        // Links sent to the old address must not work any more, except the one that undoes this
        // change.
        tx.delete_user_tokens(user.id(), TokenPurpose::PasswordReset)
            .await?;
        tx.delete_user_tokens(user.id(), TokenPurpose::MagicLink)
            .await?;
        let keep = self.own_session(&mut tx, current, user.id()).await?;
        tx.delete_user_sessions(user.id(), keep).await?;
        tx.record_audit_event(
            &self
                .ctx
                .event(user.id(), AuditAction::EmailChanged, &client)
                .detail(new_email.masked()),
        )
        .await?;
        tx.commit().await?;

        // To the address it had before, in the language it has.
        mail::notify(
            &self.ctx,
            &previous,
            Message::new("notice-email-changed").arg("email", new_email.masked()),
        )
        .await;
        tracing::info!(user_id = %user.id(), "email address changed");
        Ok(())
    }

    /// Cancels an email change from the link sent to the old address, or undoes it if it already
    /// happened, and signs out every session: the owner did not ask for it, so someone else is
    /// signed in. They should reset the password next.
    pub async fn cancel_email_change(
        &self,
        request: CancelEmailChangeRequest,
        client: ClientInfo,
    ) -> Result<(), AppError> {
        let token = request.token.0;
        if token.is_empty() || !token.is_within_limit() {
            return Err(AppError::InvalidToken);
        }
        let now = self.ctx.clock.now();

        let mut tx = self.ctx.db.transaction().await?;
        let consumed = tx
            .consume_user_token(
                &self.ctx.tokens.digest(&token),
                TokenPurpose::EmailChangeCancel,
                now,
            )
            .await?
            .ok_or(AppError::InvalidToken)?;
        let old_email = consumed.email.ok_or(AppError::InvalidToken)?;
        let user = tx
            .find_user_for_update(consumed.user_id)
            .await?
            .ok_or(AppError::InvalidToken)?;
        tx.delete_user_tokens(user.id(), TokenPurpose::EmailChange)
            .await?;
        if user.email() != &old_email {
            match tx.change_user_email(user.id(), &old_email, now).await {
                Ok(_) => {}
                Err(err) if err.is_unique_violation(EMAIL_UNIQUE_CONSTRAINT) => {
                    return Err(AppError::conflict(
                        "email_taken",
                        Message::new("conflict-old-address-taken"),
                    ));
                }
                Err(err) => return Err(err.into()),
            }
        }
        access::revoke_all(&mut tx, user.id(), None).await?;
        tx.record_audit_event(&self.ctx.event(
            user.id(),
            AuditAction::EmailChangeCancelled,
            &client,
        ))
        .await?;
        tx.commit().await?;

        let forgot = self.ctx.settings.links.forgot_password();
        mail::send(
            &*self.ctx.mailer,
            mail::email_change_cancelled(&self.ctx.voice_for(&user), old_email, &forgot),
        )
        .await;
        tracing::info!(user_id = %user.id(), "email change cancelled");
        Ok(())
    }

    async fn own_session(
        &self,
        store: &mut impl Store,
        current: Option<&Secret>,
        user: UserId,
    ) -> Result<Option<SessionId>, AppError> {
        let Some(token) = current.filter(|token| token.is_within_limit()) else {
            return Ok(None);
        };
        Ok(store
            .find_session_by_token(&self.ctx.tokens.digest(token))
            .await?
            .map(|found| found.session)
            .filter(|session| session.user_id() == user)
            .map(|session| session.id()))
    }

    async fn send_verification(&self, user: &User, token: &Secret) {
        let link = self.ctx.settings.links.verify_email(token);
        let ttl = self.ctx.settings.tokens.email_verification_ttl;
        mail::send(
            &*self.ctx.mailer,
            mail::verify_email(&self.ctx.voice_for(user), user.email().clone(), &link, ttl),
        )
        .await;
    }

    /// Tells the owner of `email` that someone tried to register it again, in their language.
    async fn send_already_registered(&self, email: &Email) -> Result<(), AppError> {
        let owner = self
            .ctx
            .db
            .connection()
            .await?
            .find_user_by_email(email)
            .await?;
        let voice = owner
            .as_ref()
            .map_or_else(|| self.ctx.voice(), |owner| self.ctx.voice_for(owner));
        let forgot = self.ctx.settings.links.forgot_password();
        mail::send(
            &*self.ctx.mailer,
            mail::already_registered(&voice, email.clone(), &forgot),
        )
        .await;
        Ok(())
    }
}

async fn find_by_identifier(
    store: &mut impl Store,
    identifier: &LoginIdentifier,
) -> Result<Option<User>, AppError> {
    Ok(match identifier {
        LoginIdentifier::Email(email) => store.find_user_by_email(email).await?,
        LoginIdentifier::Username(username) => store.find_user_by_username(username).await?,
        LoginIdentifier::Phone(phone) => store
            .find_user_by_phone(phone)
            .await?
            .filter(|user| user.verified_phone().is_some()),
    })
}
