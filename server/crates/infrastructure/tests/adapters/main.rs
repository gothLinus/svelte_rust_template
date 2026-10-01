#![expect(
    clippy::unwrap_used,
    reason = "`clippy.toml` only exempts `#[test]` functions; the helpers here may unwrap too"
)]

mod config;
mod crypto;
mod mail;
mod profiles;
mod rate_limits;
mod sign_in;
mod smtp;
mod telemetry;
mod testing;
mod twilio;
