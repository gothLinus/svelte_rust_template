use std::{sync::Arc, time::Duration};

use application::{Adapters, Services, maintenance::Cleanup};
use domain::error::ErrorChain;
use infrastructure::db::ClusterLock;
use tokio::time::{MissedTickBehavior, interval};

use crate::rate_limit::RateLimits;

pub const MAINTENANCE_INTERVAL: Duration = Duration::from_mins(10);

/// Deletes expired sessions and tokens, and prunes idle rate limit buckets, every [`MAINTENANCE_INTERVAL`], until the task is aborted. With
/// `lock`, the database cleanup runs on one instance per round, whichever gets the lock; each
/// instance still prunes its own in-memory buckets.
pub async fn maintenance<A: Adapters>(
    services: Arc<Services<A>>,
    limits: Arc<RateLimits>,
    lock: Option<ClusterLock>,
) {
    let mut ticker = interval(MAINTENANCE_INTERVAL);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

    loop {
        ticker.tick().await;
        if let Err(err) = limits.retain_recent().await {
            tracing::warn!(error = %ErrorChain(&err), "pruning rate limit buckets failed");
        }
        match &lock {
            Some(lock) => match lock.run_if_free(delete_expired(&services)).await {
                Ok(true) => {}
                Ok(false) => tracing::debug!("another instance runs maintenance this round"),
                Err(err) => tracing::warn!(error = %ErrorChain(&err), "maintenance lock failed"),
            },
            None => delete_expired(&services).await,
        }
    }
}

async fn delete_expired<A: Adapters>(services: &Services<A>) {
    match services.maintenance.delete_expired().await {
        Ok(Cleanup {
            sessions: 0,
            tokens: 0,
            accounts: 0,
            audit_events: 0,
            objects: 0,
        }) => {}
        Ok(Cleanup {
            sessions,
            tokens,
            accounts,
            audit_events,
            objects,
        }) => {
            tracing::debug!(
                sessions,
                tokens,
                accounts,
                audit_events,
                objects,
                "deleted expired rows"
            );
        }
        Err(err) => tracing::warn!(error = %ErrorChain(&err), "maintenance failed"),
    }
}
