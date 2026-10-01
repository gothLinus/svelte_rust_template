//! Use cases, authorization policies and the DTOs the API speaks.
//!
//! Services are generic over [`Adapters`], a bundle of the ports defined in `domain`. The
//! composition root in `api` picks the real implementations; tests pick in-memory fakes. Nothing
//! here names a database, a web framework or an async runtime.
//!
//! - [`crud`] and [`policy`]: the generic create/read/update/delete flow and the authorization
//!   rules it runs, reused by every resource.
//! - [`notes`]: the reference resource built on them (copy it with `just new-resource`).
//! - [`audit`]: reading back the security events the other use cases record.
//! - [`account`], [`admin`] and the sign-in features ([`auth`], [`mfa`], [`oauth`], [`passkeys`],
//!   [`passwordless`]): use cases specific to users, not meant to be copied.
//! - [`Context`], [`Adapters`], [`Services`]: the ports and settings, and every service built
//!   from them.
//! - [`AppError`]: what every use case returns on failure.
//!
//! DTOs are plain Rust types; the HTTP layer converts them to the Protocol Buffers wire format in
//! `/proto`. The `export-types` binary writes the domain's field limits for the frontend (see
//! [`types::limits`]).

pub use context::{Adapters, Context, Settings, Store};
pub use error::{AppError, ErrorChain, FieldError, InternalError, ValidationErrors};
pub use services::Services;

pub mod account;
pub mod actor;
pub mod admin;
pub mod audit;
pub mod auth;
pub mod crud;
pub mod dto;
pub mod export;
pub mod health;
pub mod mail;
pub mod maintenance;
pub mod mfa;
pub mod notes;
pub mod oauth;
pub mod pagination;
pub mod passkeys;
pub mod passwordless;
pub mod policy;
pub mod types;

mod codes;
mod context;
mod error;
mod services;
mod tokens;
