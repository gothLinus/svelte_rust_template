#![expect(
    clippy::unwrap_used,
    reason = "`clippy.toml` only exempts `#[test]` functions; the helpers here may unwrap too"
)]

mod catalog;
mod guard;
