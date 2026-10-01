//! Request metrics, against an in-memory exporter instead of a collector, and problem documents
//! while no traces are exported. Exported traces are tested in their own binary (`tests/traces.rs`),
//! which owns the process-wide subscriber.

use std::sync::Arc;

use api::{
    middleware::metrics::{self, HttpMetrics},
    problem::ApiError,
    router, telemetry,
};
use axum::{Router, body::Body, http::Request, middleware::from_fn_with_state, routing::get};
use domain::i18n::Translator;
use http_body_util::BodyExt;
use i18n::Catalog;
use opentelemetry::metrics::MeterProvider;
use opentelemetry_sdk::metrics::{
    InMemoryMetricExporter, PeriodicReader, SdkMeterProvider,
    data::{AggregatedMetrics, MetricData},
};
use serde_json::Value;
use tower::ServiceExt;

use crate::support::{Options, http_config};

/// No routes, only `router::with_middleware`, so every request is a `404` problem document.
fn app_with_middleware() -> Router {
    let translator: Arc<dyn Translator> = Arc::new(Catalog::embedded().unwrap());
    router::with_middleware(
        Router::new().fallback(|| async { ApiError::not_found() }),
        &http_config(&Options::default()),
        translator,
    )
}

fn get_request(uri: &str) -> Request<Body> {
    Request::get(uri).body(Body::empty()).unwrap()
}

#[tokio::test]
async fn without_exported_traces_problem_documents_name_none() {
    let app = app_with_middleware();

    let response = app.oneshot(get_request("/nowhere")).await.unwrap();

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let problem: Value = serde_json::from_slice(&body).unwrap();
    assert!(problem.get("traceId").is_none(), "{problem}");
}

#[tokio::test]
async fn requests_are_measured_by_route_template_method_and_status() {
    let exporter = InMemoryMetricExporter::default();
    let provider = SdkMeterProvider::builder()
        .with_reader(PeriodicReader::builder(exporter.clone()).build())
        .build();
    let app = Router::new()
        .route("/notes/{id}", get(|| async { "a note" }))
        .layer(from_fn_with_state(
            HttpMetrics::new(&provider.meter(telemetry::SCOPE)),
            metrics::record,
        ));

    for uri in ["/notes/1", "/notes/2", "/missing"] {
        app.clone().oneshot(get_request(uri)).await.unwrap();
    }
    provider.force_flush().unwrap();

    let mut durations = Vec::new();
    let mut active = Vec::new();
    for resource in exporter.get_finished_metrics().unwrap() {
        for metric in resource
            .scope_metrics()
            .flat_map(opentelemetry_sdk::metrics::data::ScopeMetrics::metrics)
        {
            match (metric.name(), metric.data()) {
                (
                    "http.server.request.duration",
                    AggregatedMetrics::F64(MetricData::Histogram(histogram)),
                ) => durations.extend(histogram.data_points().map(|point| {
                    let mut attributes: Vec<String> = point
                        .attributes()
                        .map(|kv| format!("{}={}", kv.key, kv.value))
                        .collect();
                    attributes.sort();
                    (attributes.join(" "), point.count())
                })),
                ("http.server.active_requests", AggregatedMetrics::I64(MetricData::Sum(sum))) => {
                    active.extend(
                        sum.data_points()
                            .map(opentelemetry_sdk::metrics::data::SumDataPoint::value),
                    );
                }
                _ => {}
            }
        }
    }
    durations.sort();

    assert_eq!(
        durations,
        [
            (
                "http.request.method=GET http.response.status_code=200 http.route=/notes/{id}"
                    .to_owned(),
                2
            ),
            (
                "http.request.method=GET http.response.status_code=404".to_owned(),
                1
            ),
        ]
    );
    assert_eq!(active, [0]);
}
