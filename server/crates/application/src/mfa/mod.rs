//! Two-step sign-in: authenticator apps (TOTP), recovery codes, and completing a sign-in with
//! either. Passkeys as the second step live in [`crate::passkeys`].
//!
//! A first step (password, emailed or texted code, social account) that finds a second factor
//! does not start a session. `auth::signin::complete_first_step` stores an `MfaChallenge` and
//! hands out a short-lived token (the `mfa` cookie). The client then answers through
//! [`MfaService::complete_with_totp`], [`MfaService::complete_with_recovery_code`] or a passkey,
//! and only `challenge::finish` starts the session.
//!
//! Every answer is counted against the challenge before it is checked, at most
//! `domain::mfa::MAX_MFA_ATTEMPTS` even for parallel guesses. A TOTP code works once because its
//! time step is stored; the secret is sealed under `SECRET_KEY`, bound to its user; recovery codes
//! are stored as digests and used up.
//!
//! Not a per-entity feature. To add a second factor, list it in `auth::signin::second_factors`,
//! add a `complete_with_*` method that calls `challenge::reserve` then `challenge::finish`, and
//! make enrolling and removing it require a recent sign-in.

pub use service::MfaService;

pub mod dto;

pub(crate) mod challenge;
mod service;
pub mod totp;
