//! Readiness: can the application serve requests right now? [`HealthService::ready`] pings the
//! database and the object store, and `api::routes::health` serves the answer as
//! `/health/ready`, next to `/health/live`, which checks nothing and needs no service.

use std::sync::{Arc, Mutex, PoisonError};

use domain::{clock::Clock, database::Database, error::ErrorChain, object_store::ObjectStore};
use serde::Serialize;
use time::{Duration, OffsetDateTime};

use crate::{Adapters, Context};

const READINESS_TTL: Duration = Duration::seconds(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Ok,
    Unavailable,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HealthDto {
    pub status: HealthStatus,
}

pub struct HealthService<A: Adapters> {
    ctx: Arc<Context<A>>,
    last: Mutex<Option<(OffsetDateTime, HealthStatus)>>,
}

impl<A: Adapters> HealthService<A> {
    pub fn new(ctx: Arc<Context<A>>) -> Self {
        Self {
            ctx,
            last: Mutex::new(None),
        }
    }

    /// Ready when the database and the object store answer, as of at most a second ago. A failed
    /// ping is logged and reported as `Unavailable`. Without the store, uploads and downloads
    /// fail, so a replica that cannot reach it is taken out of rotation too.
    pub async fn ready(&self) -> HealthStatus {
        let now = self.ctx.clock.now();
        let cached = *self.last.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some((_, status)) = cached.filter(|(at, _)| now - *at < READINESS_TTL) {
            return status;
        }
        let status = match self.ctx.db.ping().await {
            Ok(()) => match self.ctx.objects.ping().await {
                Ok(()) => HealthStatus::Ok,
                Err(err) => unavailable("object store", &err),
            },
            Err(err) => unavailable("database", &err),
        };
        *self.last.lock().unwrap_or_else(PoisonError::into_inner) = Some((now, status));
        status
    }
}

fn unavailable(dependency: &str, err: &(dyn std::error::Error + 'static)) -> HealthStatus {
    tracing::warn!(dependency, error = %ErrorChain(err), "readiness check failed");
    HealthStatus::Unavailable
}
