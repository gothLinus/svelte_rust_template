#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    reason = "`clippy.toml` only exempts `#[test]` functions; the helpers here may unwrap and panic too"
)]

mod account;
mod admin;
mod auth;
mod crud;
mod dto;
mod mail;
mod notes;
mod policy;
mod reauth;
mod sign_in;
mod support;
mod takeover;
mod totp;
