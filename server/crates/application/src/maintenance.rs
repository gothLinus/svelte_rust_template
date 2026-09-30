//! Housekeeping that runs on a timer: [`MaintenanceService::delete_expired`] removes expired rows,
//! and enforces retention.
//! `api::jobs::maintenance` calls it every ten minutes; with several replicas, one of them runs it
//! per round. Not a per-entity feature: add a resource's own cleanup to `delete_expired`.

use std::sync::Arc;

use domain::{
    clock::Clock, database::Database, error::StorageError, identity::IdentityRepository,
    mfa::MfaRepository, one_time_code::OneTimeCodeRepository, passkey::PasskeyRepository,
    session::SessionRepository, user::UserRepository, user_token::UserTokenRepository,
};
use time::Duration;

use crate::{Adapters, Context};

pub const TOTP_SETUP_TTL: Duration = Duration::days(1);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Cleanup {
    pub sessions: u64,
    pub tokens: u64,
    pub accounts: u64,
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
    /// retention: abandoned authenticator app setups, and accounts never verified within
    /// `UNVERIFIED_ACCOUNT_TTL`.
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
        Ok(Cleanup {
            sessions: conn.delete_expired_sessions(now, idle_cutoff).await?,
            tokens: conn.delete_expired_user_tokens(now).await?
                + conn.delete_expired_one_time_codes(now).await?
                + conn.delete_expired_oauth_flows(now).await?
                + conn.delete_expired_webauthn_challenges(now).await?
                + conn.delete_expired_mfa_challenges(now).await?
                + conn.delete_stale_totp_setups(now - TOTP_SETUP_TTL).await?,
            accounts,
        })
    }
}
