//! Route groups, one file per feature; each exposes `routes()`, the paths it serves.
//!
//! [`api_v1`] merges them into the versioned API, which `crate::router::build` mounts at `/api/v1`
//! behind the rate limit, CSRF and session layers. [`health()`] and [`reports()`] are outside it
//! and mounted at the root, so probes and browsers' violation reports need neither a session nor
//! the CSRF header.
//!
//! Every handler follows one pattern: extract (the signed-in user, path, query or
//! [`Proto`](crate::wire::Proto) body, the client address) -> call a service method with the
//! caller's `Actor` -> return a `Proto` DTO or a status code. Business rules and authorization
//! live in the service. `notes.rs` is the reference; register a new resource's routes in
//! [`api_v1`].

use application::Adapters;
use axum::Router;

use crate::state::AppState;

mod admin;
mod auth;
mod health;
mod me;
mod mfa;
mod notes;
mod oauth;
mod passkeys;
mod passwordless;
mod reauth;
mod reports;

pub fn api_v1<A: Adapters>() -> Router<AppState<A>> {
    Router::new()
        .merge(auth::routes())
        .merge(passwordless::routes())
        .merge(passkeys::routes())
        .merge(mfa::routes())
        .merge(oauth::routes())
        .merge(me::routes())
        .merge(reauth::routes())
        .merge(notes::routes())
        .merge(admin::routes())
}

pub fn health<A: Adapters>() -> Router<AppState<A>> {
    health::routes()
}

pub fn reports<A: Adapters>() -> Router<AppState<A>> {
    reports::routes()
}
