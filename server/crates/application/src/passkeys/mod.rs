//! Passkeys and security keys (WebAuthn): registering them, signing in with one alone, using one as
//! the second sign-in step, and re-authenticating with one.
//!
//! Every ceremony is two calls. The `*_options` call stores a `WebAuthnChallenge` (random bytes, a
//! purpose, optionally a user, a short TTL) and returns its id; the finishing call consumes it,
//! which can happen once, only for the same purpose and, where it names a user, only for that user.
//! Verification is done in-house: `webauthn` parses and compares client data and authenticator
//! data, the `Crypto` port checks the signature, and the stored signature counter is checked so a
//! cloned authenticator is noticed.
//!
//! Signing in with a passkey alone requires user verification (PIN or biometrics) and so skips the
//! second step; as a second step after another factor, presence is enough.
//!
//! Not a per-entity feature: extend `webauthn` / `PasskeyService` for new ceremonies.

pub use service::PasskeyService;

pub mod dto;

mod service;
pub(crate) mod webauthn;
