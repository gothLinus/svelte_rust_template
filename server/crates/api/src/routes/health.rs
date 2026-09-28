use application::{
    Adapters,
    health::{HealthDto, HealthStatus},
};
use axum::{Json, Router, extract::State, http::StatusCode, routing::get};

use crate::state::AppState;

pub fn routes<A: Adapters>() -> Router<AppState<A>> {
    Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready::<A>))
}

async fn live() -> Json<HealthDto> {
    Json(HealthDto {
        status: HealthStatus::Ok,
    })
}

/// The process can do useful work: the database answers (checked at most once a second). `503`
/// otherwise, so a load balancer stops sending traffic until it recovers.
async fn ready<A: Adapters>(State(state): State<AppState<A>>) -> (StatusCode, Json<HealthDto>) {
    let status = state.services.health.ready().await;
    let code = match status {
        HealthStatus::Ok => StatusCode::OK,
        HealthStatus::Unavailable => StatusCode::SERVICE_UNAVAILABLE,
    };
    (code, Json(HealthDto { status }))
}
