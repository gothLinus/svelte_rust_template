//! The HTTP interface and composition root.
//!
//! - [`router`] assembles routes and middleware for any [`application::Adapters`], so tests can run
//!   the real router against fakes.
//! - [`wire`] is the wire format: Protocol Buffers messages to and from the DTOs.
//! - [`app`] picks the production adapters (Postgres, Argon2id, SMTP) and runs the server;
//!   `main.rs` is a thin command-line wrapper around it.
//!
//! This is the only crate that names concrete implementations.

pub mod app;
pub mod background;
pub mod cookie;
pub mod extract;
pub mod jobs;
pub mod middleware;
pub mod problem;
pub mod rate_limit;
pub mod router;
pub mod shutdown;
pub mod spa;
pub mod state;
pub mod telemetry;
pub mod wire;

mod routes;
