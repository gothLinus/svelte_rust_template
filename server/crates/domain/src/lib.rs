//! The core of the application: entities, value objects and the ports the outer layers implement.
//!
//! This crate performs no I/O and depends on no framework. Anything that talks to the outside
//! world (storage, mail, password hashing, secret randomness, the clock) is a trait here and an
//! implementation in `infrastructure`.
//!
//! Two deliberate exceptions to "everything is a port":
//!
//! - **Ids** are `UUIDv7`s generated here ([`id::Id::generate`]). Their random bits are not
//!   secret, and it keeps every constructor infallible.
//! - **Audit timestamps** (`created_at`, `updated_at`) come from the database clock. Everything
//!   a rule depends on (expiry, idle timeouts, attempt windows) uses the injected
//!   [`clock::Clock`], so tests control it.

pub mod audit;
pub mod clock;
pub mod database;
pub mod error;
pub mod file;
pub mod i18n;
pub mod id;
pub mod identity;
pub mod mail;
pub mod mfa;
pub mod note;
pub mod object_store;
pub mod one_time_code;
pub mod pagination;
pub mod passkey;
pub mod rbac;
pub mod repository;
pub mod secret;
pub mod security;
pub mod session;
pub mod text;
pub mod unicode;
pub mod user;
pub mod user_token;
