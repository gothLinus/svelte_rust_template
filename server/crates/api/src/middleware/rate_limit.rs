//! The per-IP limit on every API request (`RATE_LIMIT_API_PER_IP`). The auth endpoints check their
//! own, tighter limits on top of it.
//!
//! It runs before the session lookup, so a client over its limit costs no database query (with
//! in-memory buckets).

use application::Adapters;
use axum::{
    extract::{Request, State},
    middleware::Next,
    response::Response,
};

use crate::{extract::Client, problem::ApiError, rate_limit::Action, state::AppState};

pub async fn limit<A: Adapters>(
    State(state): State<AppState<A>>,
    client: Client,
    request: Request,
    next: Next,
) -> Result<Response, ApiError> {
    state.limits.check_ip(Action::Api, client.ip()).await?;
    Ok(next.run(request).await)
}
