//! The object store adapter against a real S3 API: RustFS from `compose.yaml` (`just test` starts
//! it), reached through the `STORAGE_*` variables in `.env`. The tests share a bucket of their own,
//! `tests`, and each works on keys nobody else uses and removes them, so tests run side by side, a
//! failed one leaves at most an object behind, and the development bucket is left alone.

#![expect(
    clippy::unwrap_used,
    clippy::panic,
    reason = "`clippy.toml` only exempts `#[test]` functions; the helpers here may unwrap and panic too"
)]

mod store;
mod support;
