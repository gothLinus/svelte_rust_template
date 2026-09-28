use std::sync::Arc;

use argon2::{
    Algorithm, Argon2, Params, PasswordHash as PhcHash, PasswordHasher as _, PasswordVerifier,
    Version,
};
use domain::{
    secret::Secret,
    security::{HashError, PasswordHasher},
    user::PasswordHash,
};
use thiserror::Error;
use tokio::{sync::Semaphore, task::spawn_blocking};

/// Verified against when an account does not exist, so a sign-in with an unknown email takes as
/// long as one with a wrong password.
const DECOY_PASSWORD: &[u8] = b"decoy password that equalises sign-in timing";

/// Cost and concurrency settings of [`Argon2Hasher`]. A hash records its own memory, iterations
/// and parallelism, so changing them does not invalidate stored hashes: they keep verifying and
/// are re-hashed on the next sign-in (`needs_rehash`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Argon2Params {
    pub memory_kib: u32,
    pub iterations: u32,
    pub parallelism: u32,
    /// Hashes computed at once. Each holds `memory_kib` of RAM, so this bounds what a burst of
    /// sign-ins can allocate; further requests wait their turn.
    pub max_concurrent: usize,
    /// Requests that may wait for a turn. Beyond that a sign-in is refused at once
    /// ([`HashError::Busy`], a 503 with `Retry-After`) instead of queueing until the request times
    /// out. Zero means no waiting at all.
    pub max_queued: usize,
}

impl Default for Argon2Params {
    // One of OWASP's recommended Argon2id configurations: 19 MiB, 2 iterations, 1 lane.
    fn default() -> Self {
        Self {
            memory_kib: 19_456,
            iterations: 2,
            parallelism: 1,
            max_concurrent: 4,
            max_queued: 64,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Argon2Param {
    MemoryKib,
    Iterations,
    Parallelism,
    MaxConcurrent,
}

impl Argon2Params {
    /// Checks the parameters without hashing anything, and says which one is wrong. Used to reject
    /// a bad configuration at startup with a message naming the setting.
    ///
    /// # Errors
    ///
    /// Returns the offending parameter and why, when Argon2 rejects the cost settings or
    /// `max_concurrent` is zero. (`max_queued` may be any value.)
    pub fn validate(&self) -> Result<(), (Argon2Param, String)> {
        if self.max_concurrent == 0 {
            return Err((Argon2Param::MaxConcurrent, "must be at least 1".to_owned()));
        }
        Params::new(self.memory_kib, self.iterations, self.parallelism, None)
            .map(drop)
            .map_err(|err| {
                let param = match err {
                    argon2::Error::TimeTooSmall => Argon2Param::Iterations,
                    argon2::Error::ThreadsTooFew | argon2::Error::ThreadsTooMany => {
                        Argon2Param::Parallelism
                    }
                    // Memory, and anything a future argon2 release adds.
                    _ => Argon2Param::MemoryKib,
                };
                (param, format!("invalid Argon2 parameter: {err}"))
            })
    }
}

#[derive(Debug, Error)]
#[error("invalid Argon2 parameters")]
pub struct InvalidArgon2Params(#[source] argon2::Error);

/// Argon2id on the blocking thread pool, so hashing never stalls the async runtime.
///
/// New hashes are Argon2id v0x13 PHC strings (salt and parameters included). Hashing work is
/// bounded by [`Argon2Params::max_concurrent`] and [`Argon2Params::max_queued`]; beyond that `hash`
/// and `verify` fail with `HashError::Busy`. A password over the length limit never verifies and
/// never reaches Argon2.
///
/// `verify(password, None)` runs a full verification against a decoy hash computed at
/// construction, so an unknown account costs the same time as a wrong password.
#[derive(Clone)]
pub struct Argon2Hasher {
    argon2: Argon2<'static>,
    params: Params,
    decoy_hash: PasswordHash,
    permits: Arc<Semaphore>,
    admissions: Arc<Semaphore>,
}

impl Argon2Hasher {
    /// Computes the decoy hash up front, so construction takes one hash's time.
    ///
    /// # Errors
    ///
    /// Fails if the parameters are invalid (see [`Argon2Params::validate`]).
    pub fn new(params: Argon2Params) -> Result<Self, HashError> {
        let argon2_params = Params::new(
            params.memory_kib,
            params.iterations,
            params.parallelism,
            None,
        )
        .map_err(|err| HashError::new(InvalidArgon2Params(err)))?;
        let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, argon2_params.clone());
        let decoy_hash = hash_with(&argon2, DECOY_PASSWORD)?;

        Ok(Self {
            argon2,
            params: argon2_params,
            decoy_hash,
            permits: Arc::new(Semaphore::new(params.max_concurrent.max(1))),
            admissions: Arc::new(Semaphore::new(
                params
                    .max_concurrent
                    .max(1)
                    .saturating_add(params.max_queued),
            )),
        })
    }

    async fn run<T: Send + 'static>(
        &self,
        job: impl FnOnce(&Argon2<'static>) -> T + Send + 'static,
    ) -> Result<T, HashError> {
        let admission = Arc::clone(&self.admissions)
            .try_acquire_owned()
            .map_err(|_| HashError::Busy)?;
        let permit = Arc::clone(&self.permits)
            .acquire_owned()
            .await
            .map_err(HashError::new)?;
        let argon2 = self.argon2.clone();

        spawn_blocking(move || {
            let _permits = (admission, permit);
            job(&argon2)
        })
        .await
        .map_err(HashError::new)
    }
}

impl PasswordHasher for Argon2Hasher {
    async fn hash(&self, password: &Secret) -> Result<PasswordHash, HashError> {
        let password = password.clone();
        self.run(move |argon2| hash_with(argon2, password.expose().as_bytes()))
            .await?
    }

    async fn verify(
        &self,
        password: &Secret,
        hash: Option<&PasswordHash>,
    ) -> Result<bool, HashError> {
        // Oversized input is rejected before it reaches Argon2; there is nothing to hide, since the
        // limit is public.
        if !password.is_within_limit() {
            return Ok(false);
        }

        let password = password.clone();
        let exists = hash.is_some();
        let hash = hash.unwrap_or(&self.decoy_hash).clone();

        let matches = self
            .run(move |argon2| {
                PhcHash::new(hash.as_str()).is_ok_and(|parsed| {
                    argon2
                        .verify_password(password.expose().as_bytes(), &parsed)
                        .is_ok()
                })
            })
            .await?;

        Ok(exists && matches)
    }

    fn needs_rehash(&self, hash: &PasswordHash) -> bool {
        // A hash that does not even parse cannot have verified; nothing to redo.
        let Ok(parsed) = PhcHash::new(hash.as_str()) else {
            return false;
        };
        let Ok(params) = Params::try_from(&parsed) else {
            return true;
        };
        parsed.algorithm.as_str() != "argon2id"
            || parsed.version != Some(Version::V0x13 as u32)
            || params.m_cost() != self.params.m_cost()
            || params.t_cost() != self.params.t_cost()
            || params.p_cost() != self.params.p_cost()
    }
}

fn hash_with(argon2: &Argon2<'_>, password: &[u8]) -> Result<PasswordHash, HashError> {
    argon2
        .hash_password(password)
        .map(|hash| PasswordHash::new(hash.to_string()))
        .map_err(HashError::new)
}
