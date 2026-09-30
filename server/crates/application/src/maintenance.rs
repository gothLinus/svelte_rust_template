//! Housekeeping that runs on a timer: [`MaintenanceService::delete_expired`] removes expired rows,
//! and enforces retention.
//! `api::jobs::maintenance` calls it every ten minutes; with several replicas, one of them runs it
//! per round. Not a per-entity feature: add a resource's own cleanup to `delete_expired`.

use std::sync::Arc;

use domain::{
    audit::AuditRepository,
    clock::Clock,
    database::Database,
    error::{ErrorChain, StorageError},
    identity::IdentityRepository,
    mfa::MfaRepository,
    object_store::ObjectDeletionRepository,
    one_time_code::OneTimeCodeRepository,
    passkey::PasskeyRepository,
    session::SessionRepository,
    user::UserRepository,
    user_token::UserTokenRepository,
};
use time::Duration;

use crate::{Adapters, Context, files::purge_object};

pub const TOTP_SETUP_TTL: Duration = Duration::days(1);

/// How many objects one round removes from the store at most. Each is a request to the store, so
/// a larger backlog (a deleted account with many files) takes several rounds.
pub const OBJECT_PURGE_LIMIT: u32 = 1_000;

/// How long an object whose removal failed waits before the next attempt, so a store that is down
/// is not asked again for every queued object each round.
pub const OBJECT_PURGE_RETRY: Duration = Duration::hours(1);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cleanup {
    pub sessions: u64,
    pub tokens: u64,
    pub accounts: u64,
    pub audit_events: u64,
    /// Objects removed from the store: contents of deleted files and of uploads that never
    /// finished.
    pub objects: u64,
}

pub struct MaintenanceService<A: Adapters> {
    ctx: Arc<Context<A>>,
}

impl<A: Adapters> MaintenanceService<A> {
    pub fn new(ctx: Arc<Context<A>>) -> Self {
        Self { ctx }
    }

    /// Deletes expired sessions, emailed links, one-time codes, provider sign-in flows and
    /// challenges, which are already ignored, so this only keeps the tables small. Also enforces
    /// retention: abandoned authenticator app setups, accounts never verified within
    /// `UNVERIFIED_ACCOUNT_TTL`, and audit events older than `AUDIT_LOG_RETENTION`.
    pub async fn delete_expired(&self) -> Result<Cleanup, StorageError> {
        let now = self.ctx.clock.now();
        let idle_cutoff = self.ctx.settings.sessions.idle_cutoff(now);
        let mut conn = self.ctx.db.connection().await?;

        let accounts = match self.ctx.settings.unverified_account_ttl {
            Some(ttl) => conn.delete_unverified_users(now - ttl).await?,
            None => 0,
        };
        if accounts > 0 {
            tracing::info!(accounts, "deleted accounts that were never verified");
        }
        let audit_events = match self.ctx.settings.audit_retention {
            Some(retention) => conn.delete_audit_events_before(now - retention).await?,
            None => 0,
        };
        let sessions = conn.delete_expired_sessions(now, idle_cutoff).await?;
        let tokens = conn.delete_expired_user_tokens(now).await?
            + conn.delete_expired_one_time_codes(now).await?
            + conn.delete_expired_oauth_flows(now).await?
            + conn.delete_expired_webauthn_challenges(now).await?
            + conn.delete_expired_mfa_challenges(now).await?
            + conn.delete_stale_totp_setups(now - TOTP_SETUP_TTL).await?;
        drop(conn);
        Ok(Cleanup {
            sessions,
            tokens,
            accounts,
            audit_events,
            objects: self.purge_objects().await?,
        })
    }

    /// Removes up to [`OBJECT_PURGE_LIMIT`] objects that are due from the store, and returns how
    /// many. An object the store fails to remove is tried again after [`OBJECT_PURGE_RETRY`], and
    /// the round stops there: the store is most likely down, and the rest can wait.
    pub async fn purge_objects(&self) -> Result<u64, StorageError> {
        let now = self.ctx.clock.now();
        let due = self
            .ctx
            .db
            .connection()
            .await?
            .due_object_deletions(now, OBJECT_PURGE_LIMIT)
            .await?;

        let mut purged = 0;
        for key in due {
            match purge_object(&self.ctx, &key).await {
                Ok(()) => purged += 1,
                Err(err) => {
                    tracing::warn!(error = %ErrorChain(&err), %key, "removing an object failed");
                    self.ctx
                        .db
                        .connection()
                        .await?
                        .schedule_object_deletion(&key, Some(now + OBJECT_PURGE_RETRY))
                        .await?;
                    break;
                }
            }
        }
        if purged > 0 {
            tracing::debug!(purged, "removed objects from the store");
        }
        Ok(purged)
    }
}
