use domain::{
    database::StorageError,
    identity::{
        ExternalIdentity, IDENTITY_PROVIDER_UNIQUE_CONSTRAINT, IDENTITY_SUBJECT_UNIQUE_CONSTRAINT,
        IdentityId, IdentityRepository, NewIdentity, OAuthFlow,
    },
    mfa::{MfaChallenge, MfaRepository, TotpCredential},
    one_time_code::{CodePurpose, OneTimeCode, OneTimeCodeRepository},
    passkey::{
        ChallengeId, ChallengePurpose, NewPasskey, PASSKEY_CREDENTIAL_UNIQUE_CONSTRAINT, Passkey,
        PasskeyId, PasskeyName, PasskeyRepository, WebAuthnChallenge,
    },
    secret::TokenHash,
    user::UserId,
};
use time::OffsetDateTime;

use super::{
    fakes::START,
    memory::{Mem, unique},
};

fn count(n: usize) -> u64 {
    n as u64
}

impl OneTimeCodeRepository for Mem {
    async fn replace_one_time_code(&mut self, code: &OneTimeCode) -> Result<(), StorageError> {
        self.with(|state| {
            state
                .codes
                .retain(|c| !(c.user_id == code.user_id && c.purpose == code.purpose));
            state.codes.push(code.clone());
            Ok(())
        })
    }

    async fn find_one_time_code(
        &mut self,
        user: UserId,
        purpose: CodePurpose,
    ) -> Result<Option<OneTimeCode>, StorageError> {
        self.with(|state| {
            Ok(state
                .codes
                .iter()
                .find(|c| c.user_id == user && c.purpose == purpose)
                .cloned())
        })
    }

    async fn reserve_code_attempt(
        &mut self,
        user: UserId,
        purpose: CodePurpose,
        now: OffsetDateTime,
    ) -> Result<Option<OneTimeCode>, StorageError> {
        self.with(|state| {
            Ok(state
                .codes
                .iter_mut()
                .find(|c| c.user_id == user && c.purpose == purpose && c.is_live(now))
                .map(|code| {
                    code.attempts += 1;
                    code.clone()
                }))
        })
    }

    async fn delete_one_time_code(
        &mut self,
        user: UserId,
        purpose: CodePurpose,
    ) -> Result<bool, StorageError> {
        self.with(|state| {
            let before = state.codes.len();
            state
                .codes
                .retain(|c| !(c.user_id == user && c.purpose == purpose));
            Ok(state.codes.len() < before)
        })
    }

    async fn delete_expired_one_time_codes(
        &mut self,
        now: OffsetDateTime,
    ) -> Result<u64, StorageError> {
        self.with(|state| {
            let before = state.codes.len();
            state.codes.retain(|c| c.expires_at > now);
            Ok(count(before - state.codes.len()))
        })
    }
}

impl IdentityRepository for Mem {
    async fn create_identity(
        &mut self,
        identity: &NewIdentity,
    ) -> Result<ExternalIdentity, StorageError> {
        self.with(|state| {
            if state
                .identities
                .iter()
                .any(|i| i.provider == identity.provider && i.subject == identity.subject)
            {
                return Err(unique(IDENTITY_SUBJECT_UNIQUE_CONSTRAINT));
            }
            if state
                .identities
                .iter()
                .any(|i| i.user_id == identity.user_id && i.provider == identity.provider)
            {
                return Err(unique(IDENTITY_PROVIDER_UNIQUE_CONSTRAINT));
            }
            let created = ExternalIdentity {
                id: identity.id,
                user_id: identity.user_id,
                provider: identity.provider.clone(),
                subject: identity.subject.clone(),
                email: identity.email.clone(),
                created_at: START,
                last_used_at: START,
            };
            state.identities.push(created.clone());
            Ok(created)
        })
    }

    async fn find_identity(
        &mut self,
        provider: &str,
        subject: &str,
    ) -> Result<Option<ExternalIdentity>, StorageError> {
        self.with(|state| {
            Ok(state
                .identities
                .iter()
                .find(|i| i.provider == provider && i.subject == subject)
                .cloned())
        })
    }

    async fn list_user_identities(
        &mut self,
        user: UserId,
    ) -> Result<Vec<ExternalIdentity>, StorageError> {
        self.with(|state| {
            Ok(state
                .identities
                .iter()
                .filter(|i| i.user_id == user)
                .cloned()
                .collect())
        })
    }

    async fn touch_identity(
        &mut self,
        id: IdentityId,
        at: OffsetDateTime,
    ) -> Result<(), StorageError> {
        self.with(|state| {
            if let Some(identity) = state.identities.iter_mut().find(|i| i.id == id) {
                identity.last_used_at = at;
            }
            Ok(())
        })
    }

    async fn delete_user_identity(
        &mut self,
        user: UserId,
        provider: &str,
    ) -> Result<bool, StorageError> {
        self.with(|state| {
            let before = state.identities.len();
            state
                .identities
                .retain(|i| !(i.user_id == user && i.provider == provider));
            Ok(state.identities.len() < before)
        })
    }

    async fn delete_user_identities(&mut self, user: UserId) -> Result<u64, StorageError> {
        self.with(|state| {
            let before = state.identities.len();
            state.identities.retain(|i| i.user_id != user);
            Ok(count(before - state.identities.len()))
        })
    }

    async fn create_oauth_flow(&mut self, flow: &OAuthFlow) -> Result<(), StorageError> {
        self.with(|state| {
            state.oauth_flows.push(flow.clone());
            Ok(())
        })
    }

    async fn consume_oauth_flow(
        &mut self,
        state_hash: &TokenHash,
        now: OffsetDateTime,
    ) -> Result<Option<OAuthFlow>, StorageError> {
        self.with(|state| {
            let position = state
                .oauth_flows
                .iter()
                .position(|flow| &flow.state_hash == state_hash && flow.expires_at > now);
            Ok(position.map(|index| state.oauth_flows.remove(index)))
        })
    }

    async fn delete_expired_oauth_flows(
        &mut self,
        now: OffsetDateTime,
    ) -> Result<u64, StorageError> {
        self.with(|state| {
            let before = state.oauth_flows.len();
            state.oauth_flows.retain(|flow| flow.expires_at > now);
            Ok(count(before - state.oauth_flows.len()))
        })
    }
}

impl PasskeyRepository for Mem {
    async fn create_passkey(&mut self, passkey: &NewPasskey) -> Result<Passkey, StorageError> {
        self.with(|state| {
            if state
                .passkeys
                .iter()
                .any(|p| p.credential_id == passkey.credential_id)
            {
                return Err(unique(PASSKEY_CREDENTIAL_UNIQUE_CONSTRAINT));
            }
            let created = Passkey {
                id: passkey.id,
                user_id: passkey.user_id,
                credential_id: passkey.credential_id.clone(),
                public_key: passkey.public_key.clone(),
                algorithm: passkey.algorithm,
                sign_count: passkey.sign_count,
                transports: passkey.transports.clone(),
                name: passkey.name.clone(),
                created_at: START,
                last_used_at: None,
            };
            state.passkeys.push(created.clone());
            Ok(created)
        })
    }

    async fn list_user_passkeys(&mut self, user: UserId) -> Result<Vec<Passkey>, StorageError> {
        self.with(|state| {
            Ok(state
                .passkeys
                .iter()
                .filter(|p| p.user_id == user)
                .cloned()
                .collect())
        })
    }

    async fn find_passkey_by_credential(
        &mut self,
        credential_id: &[u8],
    ) -> Result<Option<Passkey>, StorageError> {
        self.with(|state| {
            Ok(state
                .passkeys
                .iter()
                .find(|p| p.credential_id == credential_id)
                .cloned())
        })
    }

    async fn record_passkey_use(
        &mut self,
        id: PasskeyId,
        sign_count: u32,
        at: OffsetDateTime,
    ) -> Result<(), StorageError> {
        self.with(|state| {
            if let Some(passkey) = state.passkeys.iter_mut().find(|p| p.id == id) {
                passkey.sign_count = sign_count;
                passkey.last_used_at = Some(at);
            }
            Ok(())
        })
    }

    async fn rename_user_passkey(
        &mut self,
        user: UserId,
        id: PasskeyId,
        name: &PasskeyName,
    ) -> Result<Option<Passkey>, StorageError> {
        self.with(|state| {
            Ok(state
                .passkeys
                .iter_mut()
                .find(|p| p.id == id && p.user_id == user)
                .map(|passkey| {
                    passkey.name = name.clone();
                    passkey.clone()
                }))
        })
    }

    async fn delete_user_passkey(
        &mut self,
        user: UserId,
        id: PasskeyId,
    ) -> Result<bool, StorageError> {
        self.with(|state| {
            let before = state.passkeys.len();
            state
                .passkeys
                .retain(|p| !(p.id == id && p.user_id == user));
            Ok(state.passkeys.len() < before)
        })
    }

    async fn delete_user_passkeys(&mut self, user: UserId) -> Result<u64, StorageError> {
        self.with(|state| {
            let before = state.passkeys.len();
            state.passkeys.retain(|p| p.user_id != user);
            Ok(count(before - state.passkeys.len()))
        })
    }

    async fn count_user_passkeys(&mut self, user: UserId) -> Result<u64, StorageError> {
        self.with(|state| {
            Ok(count(
                state.passkeys.iter().filter(|p| p.user_id == user).count(),
            ))
        })
    }

    async fn create_webauthn_challenge(
        &mut self,
        challenge: &WebAuthnChallenge,
    ) -> Result<(), StorageError> {
        self.with(|state| {
            state.webauthn_challenges.push(challenge.clone());
            Ok(())
        })
    }

    async fn consume_webauthn_challenge(
        &mut self,
        id: ChallengeId,
        purpose: ChallengePurpose,
        now: OffsetDateTime,
    ) -> Result<Option<WebAuthnChallenge>, StorageError> {
        self.with(|state| {
            let position = state
                .webauthn_challenges
                .iter()
                .position(|c| c.id == id && c.purpose == purpose && c.expires_at > now);
            Ok(position.map(|index| state.webauthn_challenges.remove(index)))
        })
    }

    async fn delete_expired_webauthn_challenges(
        &mut self,
        now: OffsetDateTime,
    ) -> Result<u64, StorageError> {
        self.with(|state| {
            let before = state.webauthn_challenges.len();
            state.webauthn_challenges.retain(|c| c.expires_at > now);
            Ok(count(before - state.webauthn_challenges.len()))
        })
    }
}

impl MfaRepository for Mem {
    async fn find_totp(&mut self, user: UserId) -> Result<Option<TotpCredential>, StorageError> {
        self.with(|state| Ok(state.totp.get(&user).cloned()))
    }

    async fn start_totp_setup(
        &mut self,
        user: UserId,
        sealed_secret: &[u8],
        at: OffsetDateTime,
    ) -> Result<bool, StorageError> {
        self.with(|state| {
            if state
                .totp
                .get(&user)
                .is_some_and(TotpCredential::is_confirmed)
            {
                return Ok(false);
            }
            state.totp.insert(
                user,
                TotpCredential {
                    user_id: user,
                    sealed_secret: sealed_secret.to_vec(),
                    confirmed_at: None,
                    last_used_step: None,
                },
            );
            state.totp_started.insert(user, at);
            Ok(true)
        })
    }

    async fn delete_stale_totp_setups(
        &mut self,
        cutoff: OffsetDateTime,
    ) -> Result<u64, StorageError> {
        self.with(|state| {
            let before = state.totp.len();
            let started = state.totp_started.clone();
            state.totp.retain(|user, totp| {
                totp.is_confirmed() || started.get(user).is_none_or(|at| *at >= cutoff)
            });
            Ok(count(before - state.totp.len()))
        })
    }

    async fn confirm_totp(
        &mut self,
        user: UserId,
        at: OffsetDateTime,
        step: i64,
    ) -> Result<bool, StorageError> {
        self.with(|state| match state.totp.get_mut(&user) {
            Some(totp) if !totp.is_confirmed() => {
                totp.confirmed_at = Some(at);
                totp.last_used_step = Some(step);
                Ok(true)
            }
            _ => Ok(false),
        })
    }

    async fn use_totp_step(&mut self, user: UserId, step: i64) -> Result<bool, StorageError> {
        self.with(|state| match state.totp.get_mut(&user) {
            Some(totp)
                if totp.is_confirmed() && totp.last_used_step.is_none_or(|last| last < step) =>
            {
                totp.last_used_step = Some(step);
                Ok(true)
            }
            _ => Ok(false),
        })
    }

    async fn delete_totp(&mut self, user: UserId) -> Result<bool, StorageError> {
        self.with(|state| Ok(state.totp.remove(&user).is_some()))
    }

    async fn replace_recovery_codes(
        &mut self,
        user: UserId,
        code_hashes: &[TokenHash],
    ) -> Result<(), StorageError> {
        self.with(|state| {
            state.recovery_codes.retain(|(owner, _)| *owner != user);
            state
                .recovery_codes
                .extend(code_hashes.iter().map(|hash| (user, *hash)));
            Ok(())
        })
    }

    async fn consume_recovery_code(
        &mut self,
        user: UserId,
        code_hash: &TokenHash,
    ) -> Result<bool, StorageError> {
        self.with(|state| {
            let before = state.recovery_codes.len();
            state
                .recovery_codes
                .retain(|(owner, hash)| !(*owner == user && hash == code_hash));
            Ok(state.recovery_codes.len() < before)
        })
    }

    async fn count_recovery_codes(&mut self, user: UserId) -> Result<u64, StorageError> {
        self.with(|state| {
            Ok(count(
                state
                    .recovery_codes
                    .iter()
                    .filter(|(owner, _)| *owner == user)
                    .count(),
            ))
        })
    }

    async fn create_mfa_challenge(&mut self, challenge: &MfaChallenge) -> Result<(), StorageError> {
        self.with(|state| {
            state.mfa_challenges.push(challenge.clone());
            Ok(())
        })
    }

    async fn find_mfa_challenge(
        &mut self,
        token_hash: &TokenHash,
    ) -> Result<Option<MfaChallenge>, StorageError> {
        self.with(|state| {
            Ok(state
                .mfa_challenges
                .iter()
                .find(|c| &c.token_hash == token_hash)
                .cloned())
        })
    }

    async fn reserve_mfa_attempt(
        &mut self,
        token_hash: &TokenHash,
        now: OffsetDateTime,
    ) -> Result<Option<MfaChallenge>, StorageError> {
        self.with(|state| {
            Ok(state
                .mfa_challenges
                .iter_mut()
                .find(|c| &c.token_hash == token_hash && c.is_live(now))
                .map(|challenge| {
                    challenge.attempts += 1;
                    challenge.clone()
                }))
        })
    }

    async fn delete_mfa_challenge(&mut self, token_hash: &TokenHash) -> Result<bool, StorageError> {
        self.with(|state| {
            let before = state.mfa_challenges.len();
            state.mfa_challenges.retain(|c| &c.token_hash != token_hash);
            Ok(state.mfa_challenges.len() < before)
        })
    }

    async fn delete_user_mfa_challenges(&mut self, user: UserId) -> Result<u64, StorageError> {
        self.with(|state| {
            let before = state.mfa_challenges.len();
            state.mfa_challenges.retain(|c| c.user_id != user);
            Ok(count(before - state.mfa_challenges.len()))
        })
    }

    async fn delete_expired_mfa_challenges(
        &mut self,
        now: OffsetDateTime,
    ) -> Result<u64, StorageError> {
        self.with(|state| {
            let before = state.mfa_challenges.len();
            state.mfa_challenges.retain(|c| c.expires_at > now);
            Ok(count(before - state.mfa_challenges.len()))
        })
    }
}
