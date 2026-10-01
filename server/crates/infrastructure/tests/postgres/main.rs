#![expect(
    clippy::unwrap_used,
    reason = "`clippy.toml` only exempts `#[test]` functions; the helpers here may unwrap too"
)]

mod audit;
mod files;
mod notes;
mod outbox;
mod pool;
mod rate_limits;
mod rbac;
mod sessions;
mod sign_in_methods;
mod support;
mod transactions;
mod user_tokens;
mod users;
