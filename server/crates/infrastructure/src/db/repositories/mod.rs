//! One module per repository trait, all implemented on [`PgExecutor`](crate::db::PgExecutor).
//!
//! The contract of each method (what `false`/`None` mean, which errors to expect) is documented on
//! the trait in `domain`; the modules here document only what the SQL adds. `notes` is the generic
//! `Repository<E>` example to copy; the others implement entity-specific traits and are not meant
//! to be copied. Several of them rely on one statement being atomic (`delete ... returning`, a
//! guarded `update`) instead of read-then-write, so concurrent requests cannot both succeed.

mod audit;
mod identities;
mod mfa;
mod notes;
mod one_time_codes;
mod passkeys;
mod rbac;
mod sessions;
mod user_tokens;
mod users;

/// How many expired rows one cleanup statement deletes. The backlog after a long outage is
/// removed in short statements, so none holds its locks (or bloats the WAL) for long. Each
/// statement re-checks the expiry in its outer `where`, so a row renewed between its selection and
/// its deletion survives.
pub(crate) const CLEANUP_BATCH: i64 = 5_000;

pub(crate) fn cleanup_done(deleted: u64) -> bool {
    deleted < CLEANUP_BATCH.unsigned_abs()
}
