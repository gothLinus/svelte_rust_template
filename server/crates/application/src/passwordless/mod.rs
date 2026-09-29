//! Signing in without a password: a magic link and a code by email, or a code by SMS or WhatsApp
//! to a verified phone number.
//!
//! `request_*` issues a single-use secret (a link token or a six-digit code, only its digest is
//! stored) and mails or texts it; `verify_*` / `magic_link` consumes it and finishes through
//! `auth::signin::complete_first_step`. Requests answer the same for unknown and known accounts,
//! and so do verifications: unknown identifiers are checked against an id no account has, so
//! timing does not differ. Wrong guesses are counted per code
//! (`domain::one_time_code::MAX_CODE_ATTEMPTS`) and the routes add rate limits on top.
//!
//! A proven email address counts as verified, which is the first proof that evicts earlier
//! claimants (see [`crate::auth`]); a texted code does not.
//!
//! Not a per-entity feature. To add a channel, extend `domain::one_time_code::CodeChannel` and the
//! `domain::text::TextSender` port; to add a way to deliver a code, follow `PasswordlessService`.

pub use service::PasswordlessService;
pub(crate) use service::{group, send_text};

pub mod dto;

mod service;
