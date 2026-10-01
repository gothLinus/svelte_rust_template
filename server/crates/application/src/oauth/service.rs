use std::sync::Arc;

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use domain::{
    audit::{AuditAction, AuditRepository, AuthMethod},
    clock::Clock,
    database::{Database, Transaction},
    i18n::Message,
    identity::{
        AuthorizationRequest, IDENTITY_PROVIDER_UNIQUE_CONSTRAINT,
        IDENTITY_SUBJECT_UNIQUE_CONSTRAINT, IdentityRepository, NewIdentity, OAUTH_FLOW_TTL,
        OAuthError, OAuthFlow, ProviderProfile,
    },
    rbac::{RbacRepository, RoleName},
    secret::{Secret, constant_time_eq},
    security::{Crypto, TokenGenerator},
    session::ClientInfo,
    user::{
        EMAIL_UNIQUE_CONSTRAINT, Email, NewUser, USERNAME_UNIQUE_CONSTRAINT, User, UserRepository,
        Username,
    },
};

use crate::{
    Adapters, Context,
    actor::Actor,
    auth::signin::{self, LoginOutcome, MfaRequired, SignedIn},
    error::{AppError, ErrorChain},
    mail,
    oauth::{
        ProviderId,
        dto::{IdentityDto, ProviderDto},
    },
};

#[derive(Debug)]
pub struct OAuthStart {
    pub url: String,
    pub state: Secret,
}

#[derive(Debug)]
pub enum OAuthOutcome {
    SignedIn(Box<SignedIn>),
    MfaRequired(MfaRequired),
    Linked,
}

#[derive(Debug)]
pub struct OAuthCallback {
    pub outcome: OAuthOutcome,
    pub redirect_to: String,
}

#[derive(Debug)]
pub struct CallbackParams<'a> {
    pub provider: &'a str,
    pub code: Option<Secret>,
    pub state: &'a str,
    pub cookie_state: Option<&'a Secret>,
    pub error: Option<&'a str>,
}

pub struct OAuthService<A: Adapters> {
    ctx: Arc<Context<A>>,
}

impl<A: Adapters> OAuthService<A> {
    pub fn new(ctx: Arc<Context<A>>) -> Self {
        Self { ctx }
    }

    pub fn providers(&self) -> Vec<ProviderDto> {
        self.ctx
            .identity_providers
            .providers()
            .iter()
            .map(|provider| ProviderDto {
                id: provider.id.clone(),
                name: provider.name.clone(),
            })
            .collect()
    }

    /// Starts signing in (or, with `linking`, linking an account) at `provider`.
    ///
    /// Stores the flow, whose PKCE verifier and nonce stay server-side (only the derived challenge
    /// and the nonce go to the provider), and returns the authorization URL. The caller must send
    /// `state` to the browser as a cookie for [`OAuthService::callback`].
    ///
    /// # Errors
    ///
    /// `NotFound` for a provider that is not configured. Linking needs a verified address and a
    /// recent sign-in. `redirect_to` is used only if it is a same-site path, otherwise the default
    /// for the flow.
    pub async fn start(
        &self,
        provider: &str,
        linking: Option<&Actor>,
        redirect_to: Option<&str>,
    ) -> Result<OAuthStart, AppError> {
        let provider = self.provider(provider)?;
        if let Some(actor) = linking {
            actor.require_verified_email()?;
            actor.require_recent_authentication()?;
        }
        let state = self.ctx.tokens.generate()?;
        let verifier = self.ctx.tokens.generate()?;
        let nonce = self.ctx.tokens.generate()?;
        let challenge =
            URL_SAFE_NO_PAD.encode(self.ctx.crypto.sha256(verifier.expose().as_bytes()));
        let default = if linking.is_some() {
            "/settings/security"
        } else {
            "/dashboard"
        };

        let flow = OAuthFlow {
            state_hash: self.ctx.tokens.digest(&state),
            provider: provider.as_str().to_owned(),
            pkce_verifier: verifier,
            nonce: nonce.expose().to_owned(),
            link_user: linking.map(|actor| actor.user_id),
            redirect_to: safe_redirect(redirect_to).unwrap_or(default).to_owned(),
            expires_at: self.ctx.clock.now() + OAUTH_FLOW_TTL,
        };
        self.ctx
            .db
            .connection()
            .await?
            .create_oauth_flow(&flow)
            .await?;

        let url = self
            .ctx
            .identity_providers
            .authorization_url(&AuthorizationRequest {
                provider: provider.as_str(),
                state: state.expose(),
                nonce: nonce.expose(),
                pkce_challenge: &challenge,
            })
            .map_err(provider_error)?;
        Ok(OAuthStart { url, state })
    }

    /// Finishes a flow when the provider sends the browser back.
    ///
    /// A linking flow is completed only by the user who started it. A sign-in flow signs in the
    /// account already linked to the provider identity, or creates one (see the module docs for
    /// when that is refused).
    ///
    /// # Errors
    ///
    /// `oauth_state_invalid` when the state is missing, differs from the cookie, is unknown,
    /// expired, already used or belongs to another provider; the flow is consumed even if the rest
    /// fails, so a callback link works once. `oauth_cancelled` when the provider reports an error.
    /// `email_required` / `email_unverified` when a new account cannot be created from the profile,
    /// `email_in_use` when the address has an account: the user signs in to it and links the
    /// provider.
    pub async fn callback(
        &self,
        params: CallbackParams<'_>,
        current: Option<&Actor>,
        client: ClientInfo,
        previous: Option<&Secret>,
    ) -> Result<OAuthCallback, AppError> {
        // The cookie binds the flow to the browser that started it. Without the check, an attacker
        // could send someone their own callback link and sign them in to the attacker's account.
        let state = Secret::new(params.state);
        if state.is_empty()
            || !state.is_within_limit()
            || params.cookie_state.is_none_or(|cookie| {
                !constant_time_eq(cookie.expose().as_bytes(), state.expose().as_bytes())
            })
        {
            return Err(invalid_state());
        }
        let flow = self
            .ctx
            .db
            .connection()
            .await?
            .consume_oauth_flow(&self.ctx.tokens.digest(&state), self.ctx.clock.now())
            .await?
            .filter(|flow| flow.provider == params.provider)
            .ok_or_else(invalid_state)?;

        if params.error.is_some() {
            return Err(AppError::conflict(
                "oauth_cancelled",
                Message::new("conflict-oauth-cancelled"),
            ));
        }
        let code = params.code.ok_or_else(invalid_state)?;
        // Checked again: the provider may have been turned off since the flow started.
        let provider = self.provider(&flow.provider)?;
        let profile = self
            .ctx
            .identity_providers
            .exchange(provider.as_str(), &code, &flow.pkce_verifier, &flow.nonce)
            .await
            .map_err(provider_error)?;

        let outcome = match flow.link_user {
            Some(user) => {
                let actor = current
                    .filter(|actor| actor.user_id == user)
                    .ok_or_else(invalid_state)?;
                actor.require_verified_email()?;
                self.link(actor, &provider, &profile).await?;
                OAuthOutcome::Linked
            }
            None => match self.sign_in(&provider, &profile, client, previous).await? {
                LoginOutcome::SignedIn(signed_in) => OAuthOutcome::SignedIn(signed_in),
                LoginOutcome::MfaRequired(required) => OAuthOutcome::MfaRequired(required),
            },
        };
        Ok(OAuthCallback {
            outcome,
            redirect_to: flow.redirect_to,
        })
    }

    pub async fn identities(&self, actor: &Actor) -> Result<Vec<IdentityDto>, AppError> {
        let identities = self
            .ctx
            .db
            .connection()
            .await?
            .list_user_identities(actor.user_id)
            .await?;
        Ok(identities
            .iter()
            .map(|identity| IdentityDto::new(identity, &self.provider_name(&identity.provider)))
            .collect())
    }

    /// Unlinks an account. Always safe, since an emailed code or link can still sign the user in.
    /// Needs a recent sign-in.
    pub async fn unlink(&self, actor: &Actor, provider: &str) -> Result<(), AppError> {
        actor.require_recent_authentication()?;
        let mut tx = self.ctx.db.transaction().await?;
        if !tx.delete_user_identity(actor.user_id, provider).await? {
            return Err(AppError::NotFound);
        }
        tx.record_audit_event(
            &self
                .ctx
                .actor_event(actor, AuditAction::IdentityUnlinked)
                .detail(provider),
        )
        .await?;
        tx.commit().await?;
        tracing::info!(user_id = %actor.user_id, provider, "social account unlinked");
        Ok(())
    }

    async fn sign_in(
        &self,
        provider: &ProviderId,
        profile: &ProviderProfile,
        client: ClientInfo,
        previous: Option<&Secret>,
    ) -> Result<LoginOutcome, AppError> {
        let now = self.ctx.clock.now();
        let mut conn = self.ctx.db.connection().await?;
        if let Some(identity) = conn
            .find_identity(provider.as_str(), &profile.subject)
            .await?
        {
            conn.touch_identity(identity.id, now).await?;
            let user = conn
                .find_user(identity.user_id)
                .await?
                .ok_or_else(invalid_state)?;
            drop(conn);
            return signin::complete_first_step(
                &self.ctx,
                &user,
                client,
                previous,
                AuthMethod::Provider(provider.as_str()),
            )
            .await;
        }
        drop(conn);

        let user = self.sign_up(provider, profile, &client).await?;
        signin::complete_first_step(
            &self.ctx,
            &user,
            client,
            previous,
            AuthMethod::Provider(provider.as_str()),
        )
        .await
    }

    /// Creates an account for someone new. An existing account with the same address is never taken
    /// over: whoever controls the provider account may not control the address. Its owner can link
    /// the provider from their settings instead.
    ///
    /// Only a provider that vouches for the address may create an account with it. Otherwise anyone
    /// could claim someone else's address through a provider that does not check it, and their
    /// provider account would stay linked after the address's owner took the account back.
    async fn sign_up(
        &self,
        provider: &ProviderId,
        profile: &ProviderProfile,
        client: &ClientInfo,
    ) -> Result<User, AppError> {
        let email = profile
            .email
            .as_deref()
            .and_then(|email| Email::parse(email).ok())
            .ok_or_else(|| {
                AppError::conflict(
                    "email_required",
                    Message::new("conflict-oauth-email-required"),
                )
            })?;
        if !profile.email_verified {
            return Err(AppError::conflict(
                "email_unverified",
                Message::new("conflict-oauth-email-unverified")
                    .arg("provider", self.provider_name(provider.as_str())),
            ));
        }
        let now = self.ctx.clock.now();
        let mut tx = self.ctx.db.transaction().await?;
        let username = self.free_username(&mut tx, &email, profile).await?;
        let user = match tx
            .create_user(&NewUser {
                id: domain::user::UserId::generate_at(now),
                email,
                username,
                password_hash: None,
                email_verified_at: Some(now),
            })
            .await
        {
            Ok(user) => user,
            Err(err) if err.is_unique_violation(EMAIL_UNIQUE_CONSTRAINT) => {
                return Err(AppError::conflict(
                    "email_in_use",
                    Message::new("conflict-oauth-email-in-use")
                        .arg("provider", self.provider_name(provider.as_str())),
                ));
            }
            // Someone registered the generated username since it was checked.
            Err(err) if err.is_unique_violation(USERNAME_UNIQUE_CONSTRAINT) => {
                return Err(AppError::conflict(
                    "username_taken",
                    Message::new("conflict-oauth-username-taken"),
                ));
            }
            Err(err) => return Err(err.into()),
        };
        tx.grant_role(user.id(), &RoleName::USER).await?;
        tx.create_identity(&NewIdentity {
            id: domain::identity::IdentityId::generate_at(now),
            user_id: user.id(),
            provider: provider.as_str().to_owned(),
            subject: profile.subject.clone(),
            email: profile.email.clone(),
        })
        .await?;
        tx.record_audit_event(
            &self
                .ctx
                .event(user.id(), AuditAction::Registered, client)
                .detail(AuthMethod::Provider(provider.as_str()).detail()),
        )
        .await?;
        tx.commit().await?;

        tracing::info!(user_id = %user.id(), %provider, "account registered through a provider");
        Ok(user)
    }

    async fn link(
        &self,
        actor: &Actor,
        provider: &ProviderId,
        profile: &ProviderProfile,
    ) -> Result<(), AppError> {
        let user = actor.user_id;
        let mut tx = self.ctx.db.transaction().await?;
        if let Some(existing) = tx
            .find_identity(provider.as_str(), &profile.subject)
            .await?
        {
            return if existing.user_id == user {
                Ok(())
            } else {
                Err(identity_taken())
            };
        }
        let linked = tx
            .create_identity(&NewIdentity {
                id: domain::identity::IdentityId::generate_at(self.ctx.clock.now()),
                user_id: user,
                provider: provider.as_str().to_owned(),
                subject: profile.subject.clone(),
                email: profile.email.clone(),
            })
            .await;
        match linked {
            Ok(_) => {}
            Err(err) if err.is_unique_violation(IDENTITY_PROVIDER_UNIQUE_CONSTRAINT) => {
                return Err(AppError::conflict(
                    "provider_linked",
                    Message::new("conflict-provider-linked"),
                ));
            }
            Err(err) if err.is_unique_violation(IDENTITY_SUBJECT_UNIQUE_CONSTRAINT) => {
                return Err(identity_taken());
            }
            Err(err) => return Err(err.into()),
        }
        tx.record_audit_event(
            &self
                .ctx
                .actor_event(actor, AuditAction::IdentityLinked)
                .detail(provider.as_str()),
        )
        .await?;
        let owner = tx.find_user(user).await?;
        tx.commit().await?;

        if let Some(owner) = owner {
            let what = Message::new("notice-identity-linked")
                .arg("provider", self.provider_name(provider.as_str()));
            mail::notify(&self.ctx, owner.email().clone(), what).await;
        }
        tracing::info!(user_id = %user, %provider, "social account linked");
        Ok(())
    }

    fn provider(&self, raw: &str) -> Result<ProviderId, AppError> {
        self.ctx
            .identity_providers
            .providers()
            .iter()
            .find(|known| known.id == raw)
            .map(|known| ProviderId(known.id.clone()))
            .ok_or(AppError::NotFound)
    }

    async fn free_username(
        &self,
        tx: &mut <A::Db as Database>::Transaction,
        email: &Email,
        profile: &ProviderProfile,
    ) -> Result<Username, AppError> {
        const ATTEMPTS: usize = 5;

        let local_part = email.as_str().split('@').next().unwrap_or_default();
        let hint = [Some(local_part), profile.name.as_deref()]
            .into_iter()
            .flatten()
            .find(|hint| Username::suggest(hint, "").is_some())
            .unwrap_or("user");

        let mut suffix = String::new();
        for _ in 0..ATTEMPTS {
            if let Some(username) = Username::suggest(hint, &suffix)
                && tx.find_user_by_username(&username).await?.is_none()
            {
                return Ok(username);
            }
            self.ctx
                .crypto
                .random_digits(4)?
                .expose()
                .clone_into(&mut suffix);
        }
        Err(AppError::conflict(
            "username_taken",
            Message::new("conflict-oauth-username-unavailable"),
        ))
    }

    fn provider_name(&self, provider: &str) -> String {
        self.ctx
            .identity_providers
            .providers()
            .iter()
            .find(|known| known.id == provider)
            .map_or_else(|| provider.to_owned(), |known| known.name.clone())
    }
}

/// `raw` if it is a path on this site. Anything else could send the user elsewhere after signing
/// in, an open redirect.
pub(crate) fn safe_redirect(raw: Option<&str>) -> Option<&str> {
    raw.filter(|path| {
        path.starts_with('/')
            && !path.starts_with("//")
            && !path.starts_with("/\\")
            && !path.starts_with("/api")
            && path.len() <= 512
            && !path.chars().any(char::is_control)
    })
}

fn invalid_state() -> AppError {
    AppError::conflict(
        "oauth_state_invalid",
        Message::new("conflict-oauth-state-invalid"),
    )
}

fn identity_taken() -> AppError {
    AppError::conflict("identity_taken", Message::new("conflict-identity-taken"))
}

fn provider_error(err: OAuthError) -> AppError {
    match err {
        OAuthError::UnknownProvider => AppError::NotFound,
        err => {
            tracing::warn!(error = %ErrorChain(&err), "social sign-in failed");
            AppError::ProviderUnavailable
        }
    }
}
