#![expect(
    clippy::unwrap_used,
    reason = "`clippy.toml` only exempts `#[test]` functions; the helpers here may unwrap too"
)]

mod errors;
mod files;
mod ids;
mod notes;
mod pagination;
mod rbac;
mod secrets;
mod sessions;
mod sign_in_methods;
mod user_tokens;
mod users;
