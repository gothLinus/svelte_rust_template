//! Request extractors.
//!
//! `Form`, `Query` and `Path` wrap axum's extractors so malformed input is rejected with the same
//! problem document as every other error. Bodies are [`Proto`] messages.

use axum::extract::{FromRequest, FromRequestParts};

use crate::problem::ApiError;
pub use crate::wire::Proto;

pub use auth::{
    Authentication, CurrentUser, OptionalUser, PermissionMarker, RequirePermission, permission,
};
pub use client::{Client, client_ip};

mod auth;
mod client;

#[derive(Debug, FromRequestParts)]
#[from_request(via(axum::extract::Query), rejection(ApiError))]
pub struct Query<T>(pub T);

/// Path parameters. One that does not parse (a malformed id) is a `404`: it cannot name an
/// existing resource.
#[derive(Debug, FromRequestParts)]
#[from_request(via(axum::extract::Path), rejection(ApiError))]
pub struct Path<T>(pub T);

/// A URL-encoded form body, for the one endpoint a provider posts to (see `routes/oauth.rs`);
/// everything else takes a [`Proto`] body.
#[derive(Debug, FromRequest)]
#[from_request(via(axum::Form), rejection(ApiError))]
pub struct Form<T>(pub T);
