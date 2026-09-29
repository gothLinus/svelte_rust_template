//! Authentication: who the caller is. Registration, password sign-in, opaque session tokens,
//! email verification, password reset, email change and step-up re-authentication.
//!
//! Not a per-entity feature, so nothing here is copied for a new resource. Extend it instead: a
//! new sign-in method ends in `signin::complete_first_step`, which runs the checks every method
//! shares, starts the second step when the user has one and otherwise starts the session (see
//! [`crate::mfa`], [`crate::oauth`], [`crate::passkeys`] and [`crate::passwordless`] for the other
//! first steps).
//!
//! - Application: [`AuthService`] (accounts, sessions, links), [`ReauthService`], the DTOs in
//!   [`dto`], and the crate-private `access` (what a proof of address ownership, a reset or "sign
//!   out everywhere" revokes) and `signin` (finishing a sign-in).
//! - Domain: `domain::user`, `domain::session` and `domain::user_token` with their repositories.
//! - Edge: `api::routes::{auth, reauth}`, `api::wire::auth`, `proto/api/v1/auth.proto`, and the
//!   `__Host-` cookies in `api::cookie` (session, registration mark, known device).
//!
//! Invariants: unknown identifiers and wrong passwords are indistinguishable, signing in always
//! issues a fresh session token, and recovery revokes sessions and everything pending.

pub use crate::tokens::KNOWN_DEVICE_TTL;
pub use reauth::ReauthService;
pub use service::{AuthService, Authenticated, Browser, LoginAccount, Registered};
pub use signin::{LoginOutcome, MfaRequired, SignedIn};

pub mod dto;

pub(crate) mod access;
pub(crate) mod reauth;
mod service;
pub(crate) mod signin;
