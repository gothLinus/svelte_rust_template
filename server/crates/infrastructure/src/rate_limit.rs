//! Where rate limit buckets live: in this process's memory, or in Postgres so that every instance
//! behind a load balancer shares them.
//!
//! Both stores run the generic cell rate algorithm (GCRA): a bucket stores only the "theoretical
//! arrival time" (TAT) of its next request. A request may arrive up to the tolerance ahead of it,
//! which allows `burst` requests at once and then one every `interval`. Which buckets exist and
//! what they are keyed on is the caller's business.

use std::{
    collections::HashMap,
    hash::{BuildHasher, RandomState},
    num::NonZeroU32,
    sync::{Arc, Mutex, MutexGuard, PoisonError},
    time::Duration,
};

use domain::{clock::Clock, error::StorageError};
use sqlx::{PgPool, postgres::types::PgInterval};
use time::OffsetDateTime;

use crate::{
    clock::SystemClock,
    db::{
        errors::db_error,
        repositories::{CLEANUP_BATCH, cleanup_done},
    },
};

/// How many buckets the memory store keeps at most, by default (`RATE_LIMIT_MEMORY_MAX_KEYS`):
/// about 100 MB. Rotating through IPv6 prefixes or addresses creates a bucket per request, and
/// buckets only go once they refilled, so without a cap a flood grows the map without bound.
pub const DEFAULT_MEMORY_MAX_KEYS: usize = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rate {
    pub burst: NonZeroU32,
    pub period: Duration,
}

impl Rate {
    pub const fn new(burst: NonZeroU32, period: Duration) -> Self {
        Self { burst, period }
    }

    fn interval(self) -> Duration {
        self.period / self.burst.get()
    }

    fn tolerance(self) -> Duration {
        self.interval() * self.burst.get()
    }
}

const fn nz(value: u32) -> NonZeroU32 {
    match NonZeroU32::new(value) {
        Some(value) => value,
        None => NonZeroU32::MIN,
    }
}

const MINUTE: Duration = Duration::from_mins(1);
const HOUR: Duration = Duration::from_hours(1);

/// The limits for each endpoint group. Each one can be overridden with a `RATE_LIMIT_<NAME>`
/// variable (see `config::http`).
#[derive(Debug, Clone, Copy)]
pub struct Rates {
    /// Every request to `/api/v1`, on top of the limits below. `None` turns it off: with
    /// `RATE_LIMIT_STORE=postgres` it costs a write per API request.
    pub api_per_ip: Option<Rate>,
    pub login_per_ip: Rate,
    /// Failed password sign-ins for one account from one IP. Successful sign-ins are not counted,
    /// so an attacker elsewhere cannot lock the owner out with this one.
    pub login_per_account: Rate,
    /// Failed password sign-ins for one account from anywhere: the ceiling on guessing one
    /// account's password from many IPs.
    pub login_per_account_total: Rate,
    pub register_per_ip: Rate,
    pub password_reset_per_ip: Rate,
    pub password_reset_per_account: Rate,
    pub verification_per_ip: Rate,
    pub verification_per_account: Rate,
    /// Emailing or texting a sign-in or verification code, and account changes that send mail (new
    /// address, password link).
    pub send_code_per_ip: Rate,
    pub send_code_per_account: Rate,
    pub check_code_per_ip: Rate,
    pub check_code_per_account: Rate,
    pub ceremony_per_ip: Rate,
    /// Content Security Policy violation reports: a page reports each violation once, so an honest
    /// client stays far below this.
    pub report_per_ip: Rate,
}

impl Default for Rates {
    fn default() -> Self {
        Self {
            // Far above what the SPA sends; it stops scripted floods, not people. Clients behind
            // one NAT share it.
            api_per_ip: Some(Rate::new(nz(300), MINUTE)),
            login_per_ip: Rate::new(nz(20), MINUTE),
            login_per_account: Rate::new(nz(10), HOUR),
            login_per_account_total: Rate::new(nz(100), HOUR),
            register_per_ip: Rate::new(nz(10), HOUR),
            password_reset_per_ip: Rate::new(nz(10), HOUR),
            password_reset_per_account: Rate::new(nz(3), HOUR),
            verification_per_ip: Rate::new(nz(20), HOUR),
            verification_per_account: Rate::new(nz(3), HOUR),
            send_code_per_ip: Rate::new(nz(20), HOUR),
            send_code_per_account: Rate::new(nz(5), HOUR),
            check_code_per_ip: Rate::new(nz(30), MINUTE),
            check_code_per_account: Rate::new(nz(10), HOUR),
            ceremony_per_ip: Rate::new(nz(60), MINUTE),
            report_per_ip: Rate::new(nz(30), MINUTE),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limited {
    pub retry_after: Duration,
}

pub enum BucketStore {
    Memory(MemoryBuckets),
    Postgres(PgPool),
}

impl BucketStore {
    pub fn memory() -> Self {
        Self::Memory(MemoryBuckets::new(
            Arc::new(SystemClock),
            DEFAULT_MEMORY_MAX_KEYS,
        ))
    }

    /// Takes one request from the bucket, or says how long until one is free.
    ///
    /// The outer `Result` is the store failing; the inner `Err(Limited)` is the request being
    /// refused, with the time to put in `Retry-After`. A refused request does not use up the
    /// bucket.
    ///
    /// # Errors
    ///
    /// With the Postgres store, a query failure.
    pub async fn take(
        &self,
        bucket: &'static str,
        key: &str,
        rate: Rate,
    ) -> Result<Result<(), Limited>, StorageError> {
        match self {
            Self::Memory(buckets) => Ok(buckets.take(bucket, key, rate)),
            Self::Postgres(pool) => take_in_postgres(pool, bucket, key, rate).await,
        }
    }

    /// Returns a request taken by [`BucketStore::take`], for requests that turned out not to count
    /// (a successful sign-in).
    pub async fn give_back(
        &self,
        bucket: &'static str,
        key: &str,
        rate: Rate,
    ) -> Result<(), StorageError> {
        match self {
            Self::Memory(buckets) => {
                buckets.give_back(bucket, key, rate);
                Ok(())
            }
            Self::Postgres(pool) => {
                sqlx::query!(
                    r#"
                    update rate_limits
                    set tat = greatest(tat - $3::interval, now())
                    where bucket = $1 and key = $2
                    "#,
                    bucket,
                    key,
                    interval(rate.interval()),
                )
                .execute(pool)
                .await
                .map_err(db_error)?;
                Ok(())
            }
        }
    }

    pub async fn retain_recent(&self) -> Result<u64, StorageError> {
        match self {
            Self::Memory(buckets) => Ok(buckets.retain_recent()),
            Self::Postgres(pool) => {
                let mut total = 0;
                loop {
                    let deleted = sqlx::query!(
                        r#"
                        delete from rate_limits
                        where (bucket, key) in (
                            select bucket, key from rate_limits where tat <= now() limit $1
                        )
                        and tat <= now()
                        "#,
                        CLEANUP_BATCH,
                    )
                    .execute(pool)
                    .await
                    .map_err(db_error)?
                    .rows_affected();
                    total += deleted;
                    if cleanup_done(deleted) {
                        return Ok(total);
                    }
                }
            }
        }
    }
}

/// One statement takes the request: it moves the TAT forward only if the new TAT is within the
/// tolerance, so concurrent requests on several instances cannot overdraw a bucket. Only a refusal
/// costs a second query, for its `Retry-After`.
async fn take_in_postgres(
    pool: &PgPool,
    bucket: &'static str,
    key: &str,
    rate: Rate,
) -> Result<Result<(), Limited>, StorageError> {
    let (step, tolerance) = (interval(rate.interval()), interval(rate.tolerance()));
    let taken = sqlx::query_scalar!(
        r#"
        insert into rate_limits as r (bucket, key, tat)
        values ($1, $2, now() + $3::interval)
        on conflict (bucket, key) do update
        set tat = greatest(r.tat, now()) + $3::interval
        where greatest(r.tat, now()) + $3::interval <= now() + $4::interval
        returning true as "taken!"
        "#,
        bucket,
        key,
        step,
        tolerance,
    )
    .fetch_optional(pool)
    .await
    .map_err(db_error)?;
    if taken.is_some() {
        return Ok(Ok(()));
    }

    let wait = sqlx::query_scalar!(
        r#"
        select (extract(epoch from
            greatest(tat, now()) + $3::interval - now() - $4::interval
        ) * 1000000)::bigint as "wait!"
        from rate_limits
        where bucket = $1 and key = $2
        "#,
        bucket,
        key,
        step,
        tolerance,
    )
    .fetch_optional(pool)
    .await
    .map_err(db_error)?
    .unwrap_or(0);
    Ok(Err(Limited {
        retry_after: Duration::from_micros(wait.max(0).unsigned_abs()),
    }))
}

/// A Postgres interval. Rates are minutes to hours, far from the bounds that fail the conversion;
/// if one did, the bucket would allow a request every microsecond.
fn interval(duration: Duration) -> PgInterval {
    PgInterval::try_from(duration).unwrap_or(PgInterval {
        months: 0,
        days: 0,
        microseconds: 1,
    })
}

const SHARDS: usize = 64;

type Shard = HashMap<(&'static str, String), OffsetDateTime>;

/// The buckets of [`BucketStore::Memory`]: each key's TAT, spread over 64 maps by a per-process
/// random hash (so no one can aim every key at one shard), and at most `max_keys` of them. Time
/// comes from the [`Clock`] port, so tests can move it.
pub struct MemoryBuckets {
    shards: Box<[Mutex<Shard>]>,
    hasher: RandomState,
    max_per_shard: usize,
    clock: Arc<dyn Clock>,
}

impl MemoryBuckets {
    /// At most `max_keys` buckets are kept; when full, the buckets closest to refilling are dropped
    /// first.
    pub fn new(clock: Arc<dyn Clock>, max_keys: usize) -> Self {
        Self {
            shards: (0..SHARDS).map(|_| Mutex::new(HashMap::new())).collect(),
            hasher: RandomState::new(),
            max_per_shard: max_keys.div_ceil(SHARDS).max(1),
            clock,
        }
    }

    pub fn len(&self) -> usize {
        self.shards.iter().map(|shard| lock(shard).len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn shard(&self, bucket: &'static str, key: &str) -> MutexGuard<'_, Shard> {
        let hash = self.hasher.hash_one((bucket, key));
        #[expect(
            clippy::cast_possible_truncation,
            reason = "only the low bits pick a shard"
        )]
        let index = hash as usize % SHARDS;
        lock(&self.shards[index])
    }

    fn take(&self, bucket: &'static str, key: &str, rate: Rate) -> Result<(), Limited> {
        let now = self.clock.now();
        let mut shard = self.shard(bucket, key);
        let entry = (bucket, key.to_owned());
        let next = shard
            .get(&entry)
            .copied()
            .filter(|at| *at > now)
            .unwrap_or(now)
            + rate.interval();
        let ahead = Duration::try_from(next - now).unwrap_or_default();
        if ahead > rate.tolerance() {
            return Err(Limited {
                retry_after: ahead.saturating_sub(rate.tolerance()),
            });
        }
        if !shard.contains_key(&entry) && shard.len() >= self.max_per_shard {
            make_room(&mut shard, now, self.max_per_shard);
        }
        shard.insert(entry, next);
        Ok(())
    }

    fn give_back(&self, bucket: &'static str, key: &str, rate: Rate) {
        let now = self.clock.now();
        if let Some(at) = self.shard(bucket, key).get_mut(&(bucket, key.to_owned())) {
            *at = (*at - rate.interval()).max(now);
        }
    }

    fn retain_recent(&self) -> u64 {
        let now = self.clock.now();
        let mut removed = 0;
        for shard in &self.shards {
            let mut shard = lock(shard);
            let before = shard.len();
            shard.retain(|_, at| *at > now);
            shard.shrink_to_fit();
            removed += before - shard.len();
        }
        u64::try_from(removed).unwrap_or(u64::MAX)
    }
}

fn make_room(shard: &mut Shard, now: OffsetDateTime, max: usize) {
    shard.retain(|_, at| *at > now);
    if shard.len() < max {
        return;
    }
    if let Some(oldest) = shard
        .iter()
        .min_by_key(|(_, at)| **at)
        .map(|(entry, _)| entry.clone())
    {
        shard.remove(&oldest);
    }
}

fn lock(shard: &Mutex<Shard>) -> MutexGuard<'_, Shard> {
    // The map holds plain timestamps, so it is consistent even after a panic.
    shard.lock().unwrap_or_else(PoisonError::into_inner)
}
