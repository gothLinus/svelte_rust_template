//! Request metrics, named after OpenTelemetry's HTTP semantic conventions so dashboards built for
//! them work as they are:
//!
//! - `http.server.request.duration` (histogram, seconds), by method, route template and status;
//! - `http.server.active_requests` (up-down counter), by method.
//!
//! The route is the template a request matched (`/api/v1/notes/{id}`), never the raw path, so ids
//! do not multiply the series; requests no route matched (the SPA, unknown paths) have none. Without
//! an exporter the meter is a no-op (see `crate::telemetry`).

use std::{sync::Arc, time::Instant};

use axum::{
    extract::{MatchedPath, Request, State},
    middleware::Next,
    response::Response,
};
use opentelemetry::{
    KeyValue,
    metrics::{Histogram, Meter, UpDownCounter},
};

/// The bucket boundaries the semantic conventions recommend for request durations, in seconds.
const DURATION_BUCKETS: [f64; 14] = [
    0.005, 0.01, 0.025, 0.05, 0.075, 0.1, 0.25, 0.5, 0.75, 1.0, 2.5, 5.0, 7.5, 10.0,
];

#[derive(Clone)]
pub struct HttpMetrics {
    duration: Histogram<f64>,
    active: UpDownCounter<i64>,
}

impl HttpMetrics {
    pub fn new(meter: &Meter) -> Arc<Self> {
        Arc::new(Self {
            duration: meter
                .f64_histogram("http.server.request.duration")
                .with_unit("s")
                .with_description("Duration of HTTP server requests.")
                .with_boundaries(DURATION_BUCKETS.to_vec())
                .build(),
            active: meter
                .i64_up_down_counter("http.server.active_requests")
                .with_unit("{request}")
                .with_description("Number of active HTTP server requests.")
                .build(),
        })
    }
}

/// Counts a request as active until dropped, so one the client abandoned (its future is dropped
/// mid-way) stops counting too.
struct Active<'a> {
    counter: &'a UpDownCounter<i64>,
    method: KeyValue,
}

impl<'a> Active<'a> {
    fn start(counter: &'a UpDownCounter<i64>, method: KeyValue) -> Self {
        counter.add(1, std::slice::from_ref(&method));
        Self { counter, method }
    }
}

impl Drop for Active<'_> {
    fn drop(&mut self) {
        self.counter.add(-1, std::slice::from_ref(&self.method));
    }
}

pub async fn record(
    State(metrics): State<Arc<HttpMetrics>>,
    request: Request,
    next: Next,
) -> Response {
    let method = KeyValue::new("http.request.method", request.method().as_str().to_owned());
    let route = request
        .extensions()
        .get::<MatchedPath>()
        .map(|path| KeyValue::new("http.route", path.as_str().to_owned()));
    let started = Instant::now();
    let active = Active::start(&metrics.active, method.clone());

    let response = next.run(request).await;

    drop(active);
    let status = KeyValue::new(
        "http.response.status_code",
        i64::from(response.status().as_u16()),
    );
    let mut attributes = vec![method, status];
    attributes.extend(route);
    metrics
        .duration
        .record(started.elapsed().as_secs_f64(), &attributes);
    response
}
