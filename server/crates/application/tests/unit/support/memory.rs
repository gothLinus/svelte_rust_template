//! An in-memory implementation of every storage port, with transactions.
//!
//! A connection works on the shared state directly. A transaction works on a copy taken when it
//! began and writes it back on commit; dropping it discards the copy, just like a rollback.
//! Concurrent transactions are last-writer-wins, which is fine for sequential unit tests.

use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};

use domain::{
    audit::{AuditEvent, AuditFilter, AuditRepository, NewAuditEvent},
    database::{Database, StorageError, Transaction},
    identity::{ExternalIdentity, OAuthFlow},
    mfa::{MfaChallenge, TotpCredential},
    note::{NewNote, Note, NoteChanges, NoteFilter, NoteId, NoteParts},
    one_time_code::OneTimeCode,
    pagination::{Cursor, Page, PageRequest},
    passkey::{Passkey, WebAuthnChallenge},
    rbac::{Permission, PermissionSet, RbacRepository, Role, RoleName},
    repository::Repository,
    secret::TokenHash,
    session::{AuthenticatedSession, Session, SessionId, SessionParts, SessionRepository},
    user::{
        EMAIL_UNIQUE_CONSTRAINT, Email, NewUser, PHONE_UNIQUE_CONSTRAINT, PasswordHash,
        PhoneNumber, USERNAME_UNIQUE_CONSTRAINT, User, UserFilter, UserId, UserParts,
        UserRepository, Username,
    },
    user_token::{ConsumedToken, TokenPurpose, UserToken, UserTokenRepository},
};
use time::OffsetDateTime;

use super::fakes::START;

#[derive(Clone)]
pub struct State {
    pub users: BTreeMap<UserId, User>,
    pub roles: BTreeMap<RoleName, Role>,
    pub user_roles: BTreeSet<(UserId, RoleName)>,
    pub sessions: BTreeMap<SessionId, Session>,
    pub tokens: Vec<UserToken>,
    pub notes: BTreeMap<NoteId, Note>,
    pub codes: Vec<OneTimeCode>,
    pub identities: Vec<ExternalIdentity>,
    pub oauth_flows: Vec<OAuthFlow>,
    pub passkeys: Vec<Passkey>,
    pub webauthn_challenges: Vec<WebAuthnChallenge>,
    pub totp: BTreeMap<UserId, TotpCredential>,
    pub totp_started: BTreeMap<UserId, OffsetDateTime>,
    pub recovery_codes: Vec<(UserId, TokenHash)>,
    pub mfa_challenges: Vec<MfaChallenge>,
    pub audit_events: Vec<AuditEvent>,
    pub broken: bool,
}

impl Default for State {
    fn default() -> Self {
        let role = |name: RoleName, permissions: PermissionSet| Role {
            description: format!("{name} role"),
            name,
            permissions,
        };
        let roles = [
            role(RoleName::ADMIN, PermissionSet::all()),
            role(RoleName::USER, domain::rbac::default_user_permissions()),
        ];

        Self {
            users: BTreeMap::new(),
            roles: roles
                .into_iter()
                .map(|role| (role.name.clone(), role))
                .collect(),
            user_roles: BTreeSet::new(),
            sessions: BTreeMap::new(),
            tokens: Vec::new(),
            notes: BTreeMap::new(),
            codes: Vec::new(),
            identities: Vec::new(),
            oauth_flows: Vec::new(),
            passkeys: Vec::new(),
            webauthn_challenges: Vec::new(),
            totp: BTreeMap::new(),
            totp_started: BTreeMap::new(),
            recovery_codes: Vec::new(),
            mfa_challenges: Vec::new(),
            audit_events: Vec::new(),
            broken: false,
        }
    }
}

impl State {
    fn permissions_of(&self, user: UserId) -> PermissionSet {
        self.user_roles
            .iter()
            .filter(|(holder, _)| *holder == user)
            .filter_map(|(_, role)| self.roles.get(role))
            .flat_map(|role| role.permissions.iter())
            .collect()
    }
}

#[derive(Clone, Default)]
pub struct MemoryDb {
    state: Arc<Mutex<State>>,
}

impl MemoryDb {
    pub fn with<R>(&self, f: impl FnOnce(&mut State) -> R) -> R {
        f(&mut self.state.lock().unwrap())
    }
}

pub struct Mem {
    shared: Arc<Mutex<State>>,
    local: Option<State>,
}

impl Mem {
    pub(super) fn with<R>(
        &mut self,
        f: impl FnOnce(&mut State) -> Result<R, StorageError>,
    ) -> Result<R, StorageError> {
        let run = |state: &mut State| {
            if state.broken {
                Err(StorageError::backend(std::io::Error::other(
                    "storage is broken",
                )))
            } else {
                f(state)
            }
        };
        match &mut self.local {
            Some(state) => run(state),
            None => run(&mut self.shared.lock().unwrap()),
        }
    }
}

impl Database for MemoryDb {
    type Connection = Mem;
    type Transaction = Mem;

    async fn connection(&self) -> Result<Mem, StorageError> {
        Ok(Mem {
            shared: Arc::clone(&self.state),
            local: None,
        })
    }

    async fn transaction(&self) -> Result<Mem, StorageError> {
        let copy = self.state.lock().unwrap().clone();
        Ok(Mem {
            shared: Arc::clone(&self.state),
            local: Some(copy),
        })
    }

    async fn ping(&self) -> Result<(), StorageError> {
        self.connection().await?.with(|_| Ok(()))
    }
}

impl Transaction for Mem {
    async fn commit(self) -> Result<(), StorageError> {
        if let Some(state) = self.local {
            *self.shared.lock().unwrap() = state;
        }
        Ok(())
    }
}

pub fn user_parts(user: &User) -> UserParts {
    UserParts {
        id: user.id(),
        email: user.email().clone(),
        username: user.username().clone(),
        phone: user.phone().cloned(),
        phone_verified_at: user.phone_verified_at(),
        password_hash: user.password_hash().cloned(),
        email_verified_at: user.email_verified_at(),
        disabled_at: user.disabled_at(),
        created_at: user.created_at(),
        updated_at: user.updated_at(),
    }
}

fn update_user(state: &mut State, id: UserId, change: impl FnOnce(&mut UserParts)) -> Option<User> {
    let user = state.users.get_mut(&id)?;
    let mut parts = user_parts(user);
    change(&mut parts);
    *user = User::from_parts(parts);
    Some(user.clone())
}

pub(super) fn unique(constraint: &str) -> StorageError {
    StorageError::UniqueViolation {
        constraint: constraint.to_owned(),
    }
}

fn session_parts(session: &Session) -> SessionParts {
    SessionParts {
        id: session.id(),
        user_id: session.user_id(),
        token_hash: *session.token_hash(),
        client: session.client().clone(),
        created_at: session.created_at(),
        last_seen_at: session.last_seen_at(),
        expires_at: session.expires_at(),
        rotation_pending: session.rotation_pending(),
        reauthenticated_at: session.reauthenticated_at(),
    }
}

fn page<T: Clone>(
    items: impl DoubleEndedIterator<Item = T>,
    request: &PageRequest,
    id: impl Fn(&T) -> uuid::Uuid,
) -> Page<T> {
    let after = request.after_id();
    let rows: Vec<T> = items
        .rev()
        .filter(|item| after.is_none_or(|after| id(item) < after))
        .take(request.fetch_limit() as usize)
        .collect();
    Page::from_rows(rows, request, |item| Cursor::from_uuid(id(item)))
}

impl UserRepository for Mem {
    async fn create_user(&mut self, new: &NewUser) -> Result<User, StorageError> {
        self.with(|state| {
            if state.users.values().any(|user| user.email() == &new.email) {
                return Err(unique(EMAIL_UNIQUE_CONSTRAINT));
            }
            if state
                .users
                .values()
                .any(|user| user.username() == &new.username)
            {
                return Err(unique(USERNAME_UNIQUE_CONSTRAINT));
            }
            let user = User::from_parts(UserParts {
                id: new.id,
                email: new.email.clone(),
                username: new.username.clone(),
                phone: None,
                phone_verified_at: None,
                password_hash: new.password_hash.clone(),
                email_verified_at: new.email_verified_at,
                disabled_at: None,
                created_at: START,
                updated_at: START,
            });
            state.users.insert(user.id(), user.clone());
            Ok(user)
        })
    }

    async fn find_user(&mut self, id: UserId) -> Result<Option<User>, StorageError> {
        self.with(|state| Ok(state.users.get(&id).cloned()))
    }

    async fn find_users(&mut self, ids: &[UserId]) -> Result<Vec<User>, StorageError> {
        self.with(|state| {
            Ok(ids
                .iter()
                .filter_map(|id| state.users.get(id).cloned())
                .collect())
        })
    }

    async fn find_user_for_update(&mut self, id: UserId) -> Result<Option<User>, StorageError> {
        self.find_user(id).await
    }

    async fn find_user_by_email(&mut self, email: &Email) -> Result<Option<User>, StorageError> {
        self.with(|state| {
            Ok(state
                .users
                .values()
                .find(|user| user.email() == email)
                .cloned())
        })
    }

    async fn find_user_by_username(
        &mut self,
        username: &Username,
    ) -> Result<Option<User>, StorageError> {
        self.with(|state| {
            Ok(state
                .users
                .values()
                .find(|user| user.username() == username)
                .cloned())
        })
    }

    async fn find_user_by_phone(
        &mut self,
        phone: &PhoneNumber,
    ) -> Result<Option<User>, StorageError> {
        self.with(|state| {
            Ok(state
                .users
                .values()
                .find(|user| user.phone() == Some(phone))
                .cloned())
        })
    }

    async fn list_users(
        &mut self,
        filter: &UserFilter,
        request: PageRequest,
    ) -> Result<Page<User>, StorageError> {
        self.with(|state| {
            let search = filter.search.as_deref().map(str::to_lowercase);
            let matching = state.users.values().filter(|user| {
                search.as_deref().is_none_or(|search| {
                    user.email().as_str().contains(search)
                        || user.username().as_str().contains(search)
                })
            });
            Ok(page(
                matching.cloned().collect::<Vec<_>>().into_iter(),
                &request,
                |user| user.id().as_uuid(),
            ))
        })
    }

    async fn set_user_username(
        &mut self,
        id: UserId,
        username: &Username,
    ) -> Result<Option<User>, StorageError> {
        self.with(|state| {
            if state
                .users
                .values()
                .any(|user| user.id() != id && user.username() == username)
            {
                return Err(unique(USERNAME_UNIQUE_CONSTRAINT));
            }
            Ok(update_user(state, id, |user| {
                user.username = username.clone();
            }))
        })
    }

    async fn change_user_email(
        &mut self,
        id: UserId,
        email: &Email,
        verified_at: OffsetDateTime,
    ) -> Result<Option<User>, StorageError> {
        self.with(|state| {
            if state
                .users
                .values()
                .any(|user| user.id() != id && user.email() == email)
            {
                return Err(unique(EMAIL_UNIQUE_CONSTRAINT));
            }
            Ok(update_user(state, id, |user| {
                user.email = email.clone();
                user.email_verified_at = Some(verified_at);
            }))
        })
    }

    async fn set_user_phone(
        &mut self,
        id: UserId,
        phone: Option<(&PhoneNumber, OffsetDateTime)>,
    ) -> Result<Option<User>, StorageError> {
        self.with(|state| {
            if let Some((phone, _)) = phone
                && state
                    .users
                    .values()
                    .any(|user| user.id() != id && user.phone() == Some(phone))
            {
                return Err(unique(PHONE_UNIQUE_CONSTRAINT));
            }
            Ok(update_user(state, id, |user| {
                user.phone = phone.map(|(phone, _)| phone.clone());
                user.phone_verified_at = phone.map(|(_, at)| at);
            }))
        })
    }

    async fn set_user_password(
        &mut self,
        id: UserId,
        hash: Option<&PasswordHash>,
    ) -> Result<bool, StorageError> {
        self.with(|state| {
            Ok(update_user(state, id, |user| user.password_hash = hash.cloned()).is_some())
        })
    }

    async fn mark_user_email_verified(
        &mut self,
        id: UserId,
        at: OffsetDateTime,
    ) -> Result<Option<User>, StorageError> {
        self.with(|state| {
            Ok(update_user(state, id, |user| {
                user.email_verified_at = user.email_verified_at.or(Some(at));
            }))
        })
    }

    async fn set_user_disabled(
        &mut self,
        id: UserId,
        at: Option<OffsetDateTime>,
    ) -> Result<Option<User>, StorageError> {
        self.with(|state| {
            Ok(update_user(state, id, |user| {
                user.disabled_at = at.map(|at| user.disabled_at.unwrap_or(at));
            }))
        })
    }

    async fn delete_user(&mut self, id: UserId) -> Result<bool, StorageError> {
        self.with(|state| Ok(delete_user(state, id)))
    }

    async fn delete_unverified_users(
        &mut self,
        cutoff: OffsetDateTime,
    ) -> Result<u64, StorageError> {
        self.with(|state| {
            let doomed: Vec<UserId> = state
                .users
                .values()
                .filter(|user| !user.is_email_verified() && user.created_at() < cutoff)
                .filter(|user| {
                    state
                        .user_roles
                        .iter()
                        .all(|(owner, role)| *owner != user.id() || *role == RoleName::USER)
                })
                .map(User::id)
                .collect();
            for id in &doomed {
                delete_user(state, *id);
            }
            Ok(u64::try_from(doomed.len()).unwrap())
        })
    }
}

fn delete_user(state: &mut State, id: UserId) -> bool {
    {
        {
            state.sessions.retain(|_, session| session.user_id() != id);
            state.tokens.retain(|token| token.user_id != id);
            state.user_roles.retain(|(user, _)| *user != id);
            state.notes.retain(|_, note| note.owner_id() != id);
            state.codes.retain(|code| code.user_id != id);
            state.identities.retain(|identity| identity.user_id != id);
            state.oauth_flows.retain(|flow| flow.link_user != Some(id));
            state.passkeys.retain(|passkey| passkey.user_id != id);
            state
                .webauthn_challenges
                .retain(|challenge| challenge.user_id != Some(id));
            state.totp.remove(&id);
            state.recovery_codes.retain(|(user, _)| *user != id);
            state
                .mfa_challenges
                .retain(|challenge| challenge.user_id != id);
            state.audit_events.retain(|event| event.user_id != id);
            state.users.remove(&id).is_some()
        }
    }
}

impl RbacRepository for Mem {
    async fn list_roles(&mut self) -> Result<Vec<Role>, StorageError> {
        self.with(|state| Ok(state.roles.values().cloned().collect()))
    }

    async fn find_role(&mut self, name: &RoleName) -> Result<Option<Role>, StorageError> {
        self.with(|state| Ok(state.roles.get(name).cloned()))
    }

    async fn roles_of_users(
        &mut self,
        users: &[UserId],
    ) -> Result<Vec<(UserId, RoleName)>, StorageError> {
        self.with(|state| {
            Ok(state
                .user_roles
                .iter()
                .filter(|(user, _)| users.contains(user))
                .cloned()
                .collect())
        })
    }

    async fn permissions_of_user(&mut self, user: UserId) -> Result<PermissionSet, StorageError> {
        self.with(|state| Ok(state.permissions_of(user)))
    }

    async fn grant_role(&mut self, user: UserId, role: &RoleName) -> Result<bool, StorageError> {
        self.with(|state| {
            if !state.roles.contains_key(role) {
                return Err(StorageError::ForeignKeyViolation {
                    constraint: "user_roles_role_fkey".to_owned(),
                });
            }
            Ok(state.user_roles.insert((user, role.clone())))
        })
    }

    async fn revoke_role(&mut self, user: UserId, role: &RoleName) -> Result<bool, StorageError> {
        self.with(|state| Ok(state.user_roles.remove(&(user, role.clone()))))
    }

    async fn count_enabled_users_with(
        &mut self,
        permission: Permission,
    ) -> Result<u64, StorageError> {
        self.with(|state| {
            Ok(state
                .users
                .values()
                .filter(|user| !user.is_disabled())
                .filter(|user| state.permissions_of(user.id()).contains(permission))
                .count() as u64)
        })
    }

    async fn lock_role_assignments(&mut self) -> Result<(), StorageError> {
        self.with(|_| Ok(()))
    }
}

impl SessionRepository for Mem {
    async fn create_session(&mut self, session: &Session) -> Result<(), StorageError> {
        self.with(|state| {
            state.sessions.insert(session.id(), session.clone());
            Ok(())
        })
    }

    async fn find_session_by_token(
        &mut self,
        token_hash: &TokenHash,
    ) -> Result<Option<AuthenticatedSession>, StorageError> {
        self.with(|state| {
            let Some(session) = state
                .sessions
                .values()
                .find(|session| session.token_hash() == token_hash)
            else {
                return Ok(None);
            };
            let Some(user) = state.users.get(&session.user_id()) else {
                return Ok(None);
            };
            Ok(Some(AuthenticatedSession {
                session: session.clone(),
                user: user.clone(),
                permissions: state.permissions_of(user.id()),
            }))
        })
    }

    async fn touch_session(&mut self, session: &Session) -> Result<bool, StorageError> {
        self.with(|state| {
            let Some(stored) = state.sessions.get_mut(&session.id()) else {
                return Ok(false);
            };
            stored.touch(session.last_seen_at());
            Ok(true)
        })
    }

    async fn mark_session_reauthenticated(
        &mut self,
        id: SessionId,
        at: OffsetDateTime,
    ) -> Result<bool, StorageError> {
        self.with(|state| {
            let Some(stored) = state.sessions.get_mut(&id) else {
                return Ok(false);
            };
            stored.reauthenticate(at);
            Ok(true)
        })
    }

    async fn rotate_session(
        &mut self,
        session: &Session,
        previous: &TokenHash,
    ) -> Result<bool, StorageError> {
        self.with(|state| match state.sessions.get_mut(&session.id()) {
            Some(stored) if stored.token_hash() == previous => {
                *stored = session.clone();
                Ok(true)
            }
            _ => Ok(false),
        })
    }

    async fn flag_user_sessions_for_rotation(&mut self, user: UserId) -> Result<u64, StorageError> {
        self.with(|state| {
            let mut flagged = 0;
            for session in state.sessions.values_mut() {
                if session.user_id() == user {
                    let mut parts = session_parts(session);
                    parts.rotation_pending = true;
                    *session = Session::from_parts(parts);
                    flagged += 1;
                }
            }
            Ok(flagged)
        })
    }

    async fn delete_session(&mut self, id: SessionId) -> Result<bool, StorageError> {
        self.with(|state| Ok(state.sessions.remove(&id).is_some()))
    }

    async fn delete_session_by_token(
        &mut self,
        token_hash: &TokenHash,
    ) -> Result<bool, StorageError> {
        self.with(|state| {
            let before = state.sessions.len();
            state
                .sessions
                .retain(|_, session| session.token_hash() != token_hash);
            Ok(state.sessions.len() < before)
        })
    }

    async fn delete_user_session(
        &mut self,
        user: UserId,
        id: SessionId,
    ) -> Result<bool, StorageError> {
        self.with(|state| {
            if state
                .sessions
                .get(&id)
                .is_some_and(|session| session.user_id() == user)
            {
                state.sessions.remove(&id);
                Ok(true)
            } else {
                Ok(false)
            }
        })
    }

    async fn delete_user_sessions(
        &mut self,
        user: UserId,
        keep: Option<SessionId>,
    ) -> Result<u64, StorageError> {
        self.with(|state| {
            let before = state.sessions.len();
            state
                .sessions
                .retain(|id, session| session.user_id() != user || Some(*id) == keep);
            Ok((before - state.sessions.len()) as u64)
        })
    }

    async fn list_user_sessions(&mut self, user: UserId) -> Result<Vec<Session>, StorageError> {
        self.with(|state| {
            let mut sessions: Vec<Session> = state
                .sessions
                .values()
                .filter(|session| session.user_id() == user)
                .cloned()
                .collect();
            sessions.sort_by_key(|session| std::cmp::Reverse(session.last_seen_at()));
            Ok(sessions)
        })
    }

    async fn delete_expired_sessions(
        &mut self,
        now: OffsetDateTime,
        idle_cutoff: OffsetDateTime,
    ) -> Result<u64, StorageError> {
        self.with(|state| {
            let before = state.sessions.len();
            state.sessions.retain(|_, session| {
                session.expires_at() > now && session.last_seen_at() > idle_cutoff
            });
            Ok((before - state.sessions.len()) as u64)
        })
    }
}

impl UserTokenRepository for Mem {
    async fn replace_user_token(&mut self, token: &UserToken) -> Result<(), StorageError> {
        self.with(|state| {
            state
                .tokens
                .retain(|t| !(t.user_id == token.user_id && t.purpose == token.purpose));
            state.tokens.push(token.clone());
            Ok(())
        })
    }

    async fn consume_user_token(
        &mut self,
        token_hash: &TokenHash,
        purpose: TokenPurpose,
        now: OffsetDateTime,
    ) -> Result<Option<ConsumedToken>, StorageError> {
        self.with(|state| {
            let position = state.tokens.iter().position(|token| {
                &token.token_hash == token_hash
                    && token.purpose == purpose
                    && token.expires_at > now
            });
            Ok(position.map(|index| {
                let token = state.tokens.remove(index);
                ConsumedToken {
                    user_id: token.user_id,
                    email: token.email,
                }
            }))
        })
    }

    async fn user_token_exists(
        &mut self,
        token_hash: &TokenHash,
        purpose: TokenPurpose,
        now: OffsetDateTime,
    ) -> Result<bool, StorageError> {
        self.with(|state| {
            Ok(state.tokens.iter().any(|token| {
                &token.token_hash == token_hash
                    && token.purpose == purpose
                    && token.expires_at > now
            }))
        })
    }

    async fn delete_user_tokens(
        &mut self,
        user: UserId,
        purpose: TokenPurpose,
    ) -> Result<u64, StorageError> {
        self.with(|state| {
            let before = state.tokens.len();
            state
                .tokens
                .retain(|token| !(token.user_id == user && token.purpose == purpose));
            Ok((before - state.tokens.len()) as u64)
        })
    }

    async fn delete_expired_user_tokens(
        &mut self,
        now: OffsetDateTime,
    ) -> Result<u64, StorageError> {
        self.with(|state| {
            let before = state.tokens.len();
            state.tokens.retain(|token| token.expires_at > now);
            Ok((before - state.tokens.len()) as u64)
        })
    }
}

impl AuditRepository for Mem {
    async fn record_audit_event(&mut self, event: &NewAuditEvent) -> Result<(), StorageError> {
        self.with(|state| {
            state.audit_events.push(AuditEvent {
                id: event.id,
                user_id: event.user_id,
                actor_id: event.actor_id,
                action: event.action,
                detail: event.detail.clone(),
                client: event.client.clone(),
                occurred_at: event.occurred_at,
            });
            Ok(())
        })
    }

    async fn list_audit_events(
        &mut self,
        filter: &AuditFilter,
        request: PageRequest,
    ) -> Result<Page<AuditEvent>, StorageError> {
        self.with(|state| {
            let mut matching: Vec<AuditEvent> = state
                .audit_events
                .iter()
                .filter(|event| filter.user_id.is_none_or(|user| event.user_id == user))
                .cloned()
                .collect();
            matching.sort_by_key(|event| event.id.as_uuid());
            Ok(page(matching.into_iter(), &request, |event| {
                event.id.as_uuid()
            }))
        })
    }

    async fn delete_audit_events_before(
        &mut self,
        cutoff: OffsetDateTime,
    ) -> Result<u64, StorageError> {
        self.with(|state| {
            let before = state.audit_events.len();
            state
                .audit_events
                .retain(|event| event.occurred_at >= cutoff);
            Ok((before - state.audit_events.len()) as u64)
        })
    }
}

impl Repository<Note> for Mem {
    async fn find_by_id(&mut self, id: NoteId) -> Result<Option<Note>, StorageError> {
        self.with(|state| Ok(state.notes.get(&id).cloned()))
    }

    async fn list(
        &mut self,
        filter: &NoteFilter,
        request: PageRequest,
    ) -> Result<Page<Note>, StorageError> {
        self.with(|state| {
            let matching: Vec<Note> = state
                .notes
                .values()
                .filter(|note| filter.owner_id.is_none_or(|owner| note.owner_id() == owner))
                .cloned()
                .collect();
            Ok(page(matching.into_iter(), &request, |note| {
                note.id().as_uuid()
            }))
        })
    }

    async fn create(&mut self, id: NoteId, input: &NewNote) -> Result<Note, StorageError> {
        self.with(|state| {
            let note = Note::from_parts(NoteParts {
                id,
                owner_id: input.owner_id,
                title: input.title.clone(),
                body: input.body.clone(),
                created_at: START,
                updated_at: START,
            });
            state.notes.insert(note.id(), note.clone());
            Ok(note)
        })
    }

    async fn update(
        &mut self,
        id: NoteId,
        changes: &NoteChanges,
    ) -> Result<Option<Note>, StorageError> {
        self.with(|state| {
            let Some(note) = state.notes.get_mut(&id) else {
                return Ok(None);
            };
            *note = Note::from_parts(NoteParts {
                id: note.id(),
                owner_id: note.owner_id(),
                title: changes
                    .title
                    .clone()
                    .unwrap_or_else(|| note.title().clone()),
                body: changes.body.clone().unwrap_or_else(|| note.body().clone()),
                created_at: note.created_at(),
                updated_at: note.updated_at(),
            });
            Ok(Some(note.clone()))
        })
    }

    async fn delete(&mut self, id: NoteId) -> Result<bool, StorageError> {
        self.with(|state| Ok(state.notes.remove(&id).is_some()))
    }
}
