use std::sync::Arc;

use domain::{
    clock::Clock,
    database::{Database, Transaction},
    i18n::Message,
    mfa::{MfaChallenge, MfaRepository},
    passkey::{
        CHALLENGE_TTL, ChallengeId, ChallengePurpose, MAX_CREDENTIAL_ID_LEN, NewPasskey, Passkey,
        PasskeyId, PasskeyName, PasskeyRepository, PublicKeyAlgorithm, WebAuthnChallenge,
    },
    secret::Secret,
    security::Crypto,
    session::ClientInfo,
    user::{User, UserId, UserRepository},
};

use crate::{
    Adapters, Context,
    actor::Actor,
    auth::signin::{self, SignedIn, second_factors},
    error::{AppError, ValidationErrors},
    mail,
    mfa::challenge,
    passkeys::{
        dto::{
            AuthenticatorSelectionDto, CredentialDescriptorDto, CredentialParameterDto,
            PasskeyAssertionRequest, PasskeyCreationOptionsDto, PasskeyDto, PasskeyRegisteredDto,
            PasskeyRequestOptionsDto, PasskeyUserDto, PublicKeyCreationOptionsDto,
            PublicKeyRequestOptionsDto, RegisterPasskeyRequest, RelyingPartyDto,
            RenamePasskeyRequest,
        },
        webauthn::{self, Expected, TYPE_CREATE, TYPE_GET, UserCheck},
    },
};

const CHALLENGE_BYTES: usize = 32;
const PUBLIC_KEY: &str = "public-key";

/// Passkey management and the WebAuthn ceremonies (see the [`passkeys`](crate::passkeys) module).
/// Adding a passkey needs a verified address and a recent sign-in, removing one a recent sign-in.
/// All failures of a ceremony are reported as [`AppError::InvalidPasskey`] without saying which
/// check failed.
pub struct PasskeyService<A: Adapters> {
    ctx: Arc<Context<A>>,
}

impl<A: Adapters> PasskeyService<A> {
    pub fn new(ctx: Arc<Context<A>>) -> Self {
        Self { ctx }
    }

    pub async fn list(&self, actor: &Actor) -> Result<Vec<PasskeyDto>, AppError> {
        let passkeys = self
            .ctx
            .db
            .connection()
            .await?
            .list_user_passkeys(actor.user_id)
            .await?;
        Ok(passkeys.iter().map(PasskeyDto::from).collect())
    }

    /// Options for adding a passkey to the actor's account. Existing ones are excluded, so the same
    /// authenticator is not registered twice.
    pub async fn registration_options(
        &self,
        actor: &Actor,
    ) -> Result<PasskeyCreationOptionsDto, AppError> {
        actor.require_verified_email()?;
        actor.require_recent_authentication()?;
        let mut conn = self.ctx.db.connection().await?;
        let user = conn
            .find_user(actor.user_id)
            .await?
            .ok_or(AppError::Unauthenticated)?;
        let existing = conn.list_user_passkeys(user.id()).await?;
        let challenge = self
            .new_challenge(&mut conn, ChallengePurpose::Registration, Some(user.id()))
            .await?;

        Ok(PasskeyCreationOptionsDto {
            challenge_id: challenge.id.as_uuid(),
            public_key: PublicKeyCreationOptionsDto {
                rp: RelyingPartyDto {
                    id: self.rp_id().to_owned(),
                    name: self.ctx.settings.app_name.clone(),
                },
                user: PasskeyUserDto {
                    id: user.id().as_uuid().as_bytes().to_vec(),
                    name: user.username().to_string(),
                    display_name: user.username().to_string(),
                },
                challenge: challenge.challenge.clone(),
                pub_key_cred_params: PublicKeyAlgorithm::ALL
                    .into_iter()
                    .map(|alg| CredentialParameterDto {
                        kind: PUBLIC_KEY.to_owned(),
                        alg: alg.cose(),
                    })
                    .collect(),
                timeout: timeout_ms(),
                exclude_credentials: existing.iter().map(descriptor).collect(),
                authenticator_selection: AuthenticatorSelectionDto {
                    resident_key: "preferred".to_owned(),
                    user_verification: "preferred".to_owned(),
                },
                attestation: "none".to_owned(),
            },
        })
    }

    /// Stores a new passkey. Returns recovery codes if this turned two-step sign-in on.
    ///
    /// The challenge is consumed first and even when verification then fails, so a challenge gets
    /// one try. The client data must be a `webauthn.create` for this origin, the authenticator data
    /// must be for this relying party with user presence, and the credential id and public key must
    /// match what the authenticator data attested.
    ///
    /// # Errors
    ///
    /// [`AppError::InvalidPasskey`] for any failed check, an unknown, used, expired or foreign
    /// challenge, `409 passkey_exists` for a credential registered already.
    pub async fn register(
        &self,
        actor: &Actor,
        request: RegisterPasskeyRequest,
    ) -> Result<PasskeyRegisteredDto, AppError> {
        actor.require_verified_email()?;
        actor.require_recent_authentication()?;
        let name = PasskeyName::parse(&request.name)
            .map_err(|err| ValidationErrors::single("name", &err))?;
        let now = self.ctx.clock.now();
        // Consumed outside the transaction below, so a failed attempt still uses it up.
        let challenge = self
            .ctx
            .db
            .connection()
            .await?
            .consume_webauthn_challenge(
                ChallengeId::from_uuid(request.challenge_id),
                ChallengePurpose::Registration,
                now,
            )
            .await?
            .filter(|challenge| challenge.user_id == Some(actor.user_id))
            .ok_or(AppError::InvalidPasskey)?;

        let credential_id = webauthn::field(&request.credential_id)?;
        let client_data = webauthn::field(&request.client_data_json)?;
        let authenticator_data = webauthn::field(&request.authenticator_data)?;
        let public_key = webauthn::field(&request.public_key)?;
        let algorithm = PublicKeyAlgorithm::from_cose(request.public_key_algorithm)
            .ok_or(AppError::InvalidPasskey)?;

        let data = webauthn::verify(
            &self.ctx.crypto,
            &Expected {
                kind: TYPE_CREATE,
                challenge: &challenge.challenge,
                origin: self.ctx.settings.links.origin(),
                rp_id: self.rp_id(),
                user_check: UserCheck::Presence,
            },
            &client_data,
            &authenticator_data,
        )?;
        if credential_id.is_empty()
            || credential_id.len() > MAX_CREDENTIAL_ID_LEN
            || data.credential_id != Some(credential_id.as_slice())
            || !self.ctx.crypto.is_valid_public_key(algorithm, &public_key)
        {
            return Err(AppError::InvalidPasskey);
        }

        let mut tx = self.ctx.db.transaction().await?;
        let first_factor = second_factors(&mut tx, actor.user_id).await?.is_empty();
        let passkey = tx
            .create_passkey(&NewPasskey {
                id: domain::passkey::PasskeyId::generate_at(self.ctx.clock.now()),
                user_id: actor.user_id,
                credential_id,
                public_key,
                algorithm,
                sign_count: data.sign_count,
                transports: request
                    .transports
                    .into_iter()
                    .filter(|transport| transport.len() <= 32)
                    .take(8)
                    .collect(),
                name,
            })
            .await
            .map_err(|err| {
                if err.is_unique_violation(domain::passkey::PASSKEY_CREDENTIAL_UNIQUE_CONSTRAINT) {
                    AppError::conflict("passkey_exists", Message::new("conflict-passkey-exists"))
                } else {
                    err.into()
                }
            })?;
        let recovery_codes = if first_factor {
            Some(challenge::new_recovery_codes(&self.ctx, &mut tx, actor.user_id).await?)
        } else {
            None
        };
        tx.commit().await?;

        self.notify(
            actor.user_id,
            Message::new("notice-passkey-added").arg("name", passkey.name.as_str()),
        )
        .await;
        tracing::info!(user_id = %actor.user_id, passkey_id = %passkey.id, "passkey added");
        Ok(PasskeyRegisteredDto {
            passkey: PasskeyDto::from(&passkey),
            recovery_codes,
        })
    }

    pub async fn rename(
        &self,
        actor: &Actor,
        id: PasskeyId,
        request: RenamePasskeyRequest,
    ) -> Result<PasskeyDto, AppError> {
        let name = PasskeyName::parse(&request.name)
            .map_err(|err| ValidationErrors::single("name", &err))?;
        let passkey = self
            .ctx
            .db
            .connection()
            .await?
            .rename_user_passkey(actor.user_id, id, &name)
            .await?
            .ok_or(AppError::NotFound)?;
        Ok(PasskeyDto::from(&passkey))
    }

    pub async fn delete(&self, actor: &Actor, id: PasskeyId) -> Result<(), AppError> {
        actor.require_recent_authentication()?;
        let mut tx = self.ctx.db.transaction().await?;
        if !tx.delete_user_passkey(actor.user_id, id).await? {
            return Err(AppError::NotFound);
        }
        if second_factors(&mut tx, actor.user_id).await?.is_empty() {
            tx.replace_recovery_codes(actor.user_id, &[]).await?;
        }
        tx.commit().await?;

        self.notify(actor.user_id, Message::new("notice-passkey-removed"))
            .await;
        tracing::info!(user_id = %actor.user_id, passkey_id = %id, "passkey removed");
        Ok(())
    }

    /// Options for signing in with a passkey. No account is named: the authenticator offers the
    /// passkeys it has for this site.
    pub async fn login_options(&self) -> Result<PasskeyRequestOptionsDto, AppError> {
        let mut conn = self.ctx.db.connection().await?;
        let challenge = self
            .new_challenge(&mut conn, ChallengePurpose::Authentication, None)
            .await?;
        Ok(self.request_options(&challenge, Vec::new(), "required"))
    }

    /// Signs in with a passkey. It proves both the device and its unlock (fingerprint, face, PIN),
    /// so no second step follows; the assertion must carry the user-verified flag.
    ///
    /// # Errors
    ///
    /// [`AppError::InvalidPasskey`] for a bad challenge, unknown credential, bad signature or a
    /// signature counter that did not increase; the account errors of `signin::ensure_can_sign_in`
    /// after the assertion checked out.
    pub async fn login(
        &self,
        request: PasskeyAssertionRequest,
        client: ClientInfo,
        previous: Option<&Secret>,
    ) -> Result<SignedIn, AppError> {
        let now = self.ctx.clock.now();
        let challenge = self
            .ctx
            .db
            .connection()
            .await?
            .consume_webauthn_challenge(
                ChallengeId::from_uuid(request.challenge_id),
                ChallengePurpose::Authentication,
                now,
            )
            .await?
            .ok_or(AppError::InvalidPasskey)?;
        let user = self
            .verify_assertion(&challenge, &request, UserCheck::Verification)
            .await?;
        signin::ensure_can_sign_in(&self.ctx, &user)?;

        let mut tx = self.ctx.db.transaction().await?;
        let signed_in =
            signin::start_session(&self.ctx, &mut tx, &user, client, previous, now).await?;
        tx.commit().await?;
        Ok(signed_in)
    }

    /// Options for using a passkey as the second step of the sign-in behind `mfa_token`, limited to
    /// that user's passkeys.
    pub async fn second_step_options(
        &self,
        mfa_token: &Secret,
    ) -> Result<PasskeyRequestOptionsDto, AppError> {
        let pending = challenge::load(&self.ctx, mfa_token).await?;
        let mut conn = self.ctx.db.connection().await?;
        let passkeys = conn.list_user_passkeys(pending.user_id).await?;
        if passkeys.is_empty() {
            return Err(AppError::NotFound);
        }
        let challenge = self
            .new_challenge(
                &mut conn,
                ChallengePurpose::SecondFactor,
                Some(pending.user_id),
            )
            .await?;
        Ok(self.request_options(
            &challenge,
            passkeys.iter().map(descriptor).collect(),
            "preferred",
        ))
    }

    /// Finishes the sign-in behind `mfa_token` with a passkey assertion. Counts as an attempt on
    /// the pending sign-in first, like the other second steps; the passkey must belong to the user
    /// who passed the first step.
    pub async fn complete_second_step(
        &self,
        mfa_token: &Secret,
        request: PasskeyAssertionRequest,
        client: ClientInfo,
        previous: Option<&Secret>,
    ) -> Result<SignedIn, AppError> {
        let pending: MfaChallenge = challenge::reserve(&self.ctx, mfa_token).await?;
        let webauthn_challenge = self
            .ctx
            .db
            .connection()
            .await?
            .consume_webauthn_challenge(
                ChallengeId::from_uuid(request.challenge_id),
                ChallengePurpose::SecondFactor,
                self.ctx.clock.now(),
            )
            .await?
            .filter(|challenge| challenge.user_id == Some(pending.user_id));

        let verified = match webauthn_challenge {
            Some(webauthn_challenge) => self
                .verify_assertion(&webauthn_challenge, &request, UserCheck::Presence)
                .await
                .ok()
                .filter(|user| user.id() == pending.user_id),
            None => None,
        };
        if verified.is_none() {
            return Err(AppError::InvalidPasskey);
        }
        challenge::finish(&self.ctx, &pending, client, previous).await
    }

    /// Options for re-authenticating the actor with one of their passkeys, with user verification
    /// required.
    pub async fn reauthentication_options(
        &self,
        actor: &Actor,
    ) -> Result<PasskeyRequestOptionsDto, AppError> {
        let mut conn = self.ctx.db.connection().await?;
        let passkeys = conn.list_user_passkeys(actor.user_id).await?;
        if passkeys.is_empty() {
            return Err(AppError::NotFound);
        }
        let challenge = self
            .new_challenge(
                &mut conn,
                ChallengePurpose::Reauthentication,
                Some(actor.user_id),
            )
            .await?;
        // It stands in for the password, so the device must verify the user too.
        Ok(self.request_options(
            &challenge,
            passkeys.iter().map(descriptor).collect(),
            "required",
        ))
    }

    /// Marks the actor's session re-authenticated after a passkey assertion with user verification.
    /// The passkey must belong to the actor.
    pub async fn reauthenticate(
        &self,
        actor: &Actor,
        request: PasskeyAssertionRequest,
    ) -> Result<(), AppError> {
        let challenge = self
            .ctx
            .db
            .connection()
            .await?
            .consume_webauthn_challenge(
                ChallengeId::from_uuid(request.challenge_id),
                ChallengePurpose::Reauthentication,
                self.ctx.clock.now(),
            )
            .await?
            .filter(|challenge| challenge.user_id == Some(actor.user_id))
            .ok_or(AppError::InvalidPasskey)?;
        let user = self
            .verify_assertion(&challenge, &request, UserCheck::Verification)
            .await?;
        if user.id() != actor.user_id {
            return Err(AppError::InvalidPasskey);
        }
        crate::auth::reauth::mark(&self.ctx, actor).await
    }

    /// Checks an assertion and returns the passkey's user. The challenge is already consumed by the
    /// caller.
    ///
    /// Beyond the client and authenticator data, it checks the signature over the authenticator
    /// data and client data hash with the stored public key, and the signature counter: one that
    /// did not increase (unless both are 0, as with synced passkeys) suggests a cloned
    /// authenticator and is refused (WebAuthn section 7.2). Only then is the counter stored.
    async fn verify_assertion(
        &self,
        challenge: &WebAuthnChallenge,
        request: &PasskeyAssertionRequest,
        user_check: UserCheck,
    ) -> Result<User, AppError> {
        let credential_id = webauthn::field(&request.credential_id)?;
        let client_data = webauthn::field(&request.client_data_json)?;
        let authenticator_data = webauthn::field(&request.authenticator_data)?;
        let signature = webauthn::field(&request.signature)?;

        let mut conn = self.ctx.db.connection().await?;
        let passkey: Passkey = conn
            .find_passkey_by_credential(&credential_id)
            .await?
            .ok_or(AppError::InvalidPasskey)?;
        if challenge
            .user_id
            .is_some_and(|user| user != passkey.user_id)
        {
            return Err(AppError::InvalidPasskey);
        }
        if let Some(handle) = &request.user_handle {
            let handle = webauthn::field(handle)?;
            if handle != passkey.user_id.as_uuid().as_bytes() {
                return Err(AppError::InvalidPasskey);
            }
        }

        let data = webauthn::verify(
            &self.ctx.crypto,
            &Expected {
                kind: TYPE_GET,
                challenge: &challenge.challenge,
                origin: self.ctx.settings.links.origin(),
                rp_id: self.rp_id(),
                user_check,
            },
            &client_data,
            &authenticator_data,
        )?;
        let message = webauthn::signed_message(&self.ctx.crypto, &authenticator_data, &client_data);
        if !self.ctx.crypto.verify_signature(
            passkey.algorithm,
            &passkey.public_key,
            &message,
            &signature,
        ) {
            return Err(AppError::InvalidPasskey);
        }
        if !passkey.accepts_sign_count(data.sign_count) {
            tracing::warn!(
                user_id = %passkey.user_id,
                passkey_id = %passkey.id,
                "passkey signature counter went backwards; it may have been cloned"
            );
            return Err(AppError::InvalidPasskey);
        }

        conn.record_passkey_use(passkey.id, data.sign_count, self.ctx.clock.now())
            .await?;
        conn.find_user(passkey.user_id)
            .await?
            .ok_or(AppError::InvalidPasskey)
    }

    async fn new_challenge(
        &self,
        store: &mut impl PasskeyRepository,
        purpose: ChallengePurpose,
        user_id: Option<UserId>,
    ) -> Result<WebAuthnChallenge, AppError> {
        let now = self.ctx.clock.now();
        let challenge = WebAuthnChallenge {
            id: ChallengeId::generate_at(now),
            challenge: self.ctx.crypto.random_bytes(CHALLENGE_BYTES)?,
            purpose,
            user_id,
            expires_at: now + CHALLENGE_TTL,
        };
        store.create_webauthn_challenge(&challenge).await?;
        Ok(challenge)
    }

    fn request_options(
        &self,
        challenge: &WebAuthnChallenge,
        allow_credentials: Vec<CredentialDescriptorDto>,
        user_verification: &str,
    ) -> PasskeyRequestOptionsDto {
        PasskeyRequestOptionsDto {
            challenge_id: challenge.id.as_uuid(),
            public_key: PublicKeyRequestOptionsDto {
                challenge: challenge.challenge.clone(),
                rp_id: self.rp_id().to_owned(),
                timeout: timeout_ms(),
                user_verification: user_verification.to_owned(),
                allow_credentials,
            },
        }
    }

    fn rp_id(&self) -> &str {
        self.ctx.settings.links.host()
    }

    async fn notify(&self, user: UserId, what: Message) {
        let Ok(Some(user)) = async { self.ctx.db.connection().await?.find_user(user).await }.await
        else {
            return;
        };
        mail::notify(&self.ctx, user.email().clone(), what).await;
    }
}

fn descriptor(passkey: &Passkey) -> CredentialDescriptorDto {
    CredentialDescriptorDto {
        kind: PUBLIC_KEY.to_owned(),
        id: passkey.credential_id.clone(),
        transports: passkey.transports.clone(),
    }
}

fn timeout_ms() -> u32 {
    u32::try_from(CHALLENGE_TTL.whole_milliseconds()).unwrap_or(u32::MAX)
}
