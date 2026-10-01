//! Adapters for the ports defined in `domain`: PostgreSQL through sqlx, Argon2id password hashing,
//! tokens and other cryptography (ring), SMTP mail through lettre, text messages through Twilio,
//! social sign-in over OAuth 2.0, the
//! system clock, configuration from the environment, and export of traces and metrics over OTLP.
//!
//! Only this crate and the composition root (`api`) know these libraries exist.

pub mod clock;
pub mod config;
pub mod crypto;
pub mod db;
pub mod mail;
pub mod oauth;
pub mod outbox;
pub mod rate_limit;
pub mod telemetry;
pub mod text;

#[cfg(feature = "test-support")]
pub mod testing;
