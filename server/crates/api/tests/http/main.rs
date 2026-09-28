//! The HTTP API end to end: requests go through the full router and middleware stack, the
//! application services and a real PostgreSQL. Each `#[sqlx::test]` gets its own freshly migrated
//! database. Needs `DATABASE_URL` (`just test` starts Postgres).

#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "`clippy.toml` only exempts `#[test]` functions; the helpers here may unwrap and panic too"
)]

mod abuse;
mod account;
mod admin;
mod auth;
mod errors;
mod health;
mod i18n;
mod mfa;
mod notes;
mod rate_limits;
mod reauth;
mod recovery;
mod security;
mod sign_in;
mod spa;
mod startup;
mod support;
