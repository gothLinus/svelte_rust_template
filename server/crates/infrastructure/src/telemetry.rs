//! Export of traces and metrics to an OpenTelemetry collector over OTLP/HTTP (protobuf), when
//! `OTEL_EXPORTER_OTLP_ENDPOINT` is set (see `config::telemetry`).
//!
//! [`Otlp::start`] builds a tracer and a meter provider. Each exports in batches from a thread of
//! its own, outside the async runtime, so a slow or absent collector never holds up a request: a
//! full queue drops spans, and a failed export is logged by the SDK and retried with the next
//! batch. [`Otlp::shutdown`] flushes both before the process exits.

use std::{collections::HashMap, time::Duration};

use opentelemetry_otlp::{
    MetricExporter, Protocol, SpanExporter, WithExportConfig, WithHttpConfig,
};
use opentelemetry_sdk::{
    Resource,
    metrics::SdkMeterProvider,
    trace::{Sampler, SdkTracerProvider},
};
use thiserror::Error;

use crate::{
    config::OtlpConfig,
    oauth::{USER_AGENT, tls_config},
};

/// How long one export may take before the SDK gives up on it.
const EXPORT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Error)]
pub enum OtlpError {
    #[error("the OTLP HTTP client could not be built")]
    Client(#[source] reqwest::Error),
    #[error("the OTLP exporter could not be built")]
    Exporter(#[source] opentelemetry_otlp::ExporterBuildError),
}

pub struct Otlp {
    pub tracer_provider: SdkTracerProvider,
    pub meter_provider: SdkMeterProvider,
}

impl Otlp {
    /// Builds both providers. Must run outside the async runtime: the blocking HTTP client the
    /// export threads use cannot be created, or dropped, on one of its threads.
    pub fn start(config: &OtlpConfig) -> Result<Self, OtlpError> {
        let resource = Resource::builder()
            .with_service_name(config.service_name.clone())
            .build();
        let headers: HashMap<String, String> = config
            .headers
            .iter()
            .map(|(name, value)| (name.clone(), value.expose().to_owned()))
            .collect();

        let spans = SpanExporter::builder()
            .with_http()
            .with_http_client(client()?)
            .with_protocol(Protocol::HttpBinary)
            .with_endpoint(config.signal_url("traces"))
            .with_headers(headers.clone())
            .with_timeout(EXPORT_TIMEOUT)
            .build()
            .map_err(OtlpError::Exporter)?;
        let tracer_provider = SdkTracerProvider::builder()
            .with_batch_exporter(spans)
            .with_sampler(Sampler::ParentBased(Box::new(Sampler::TraceIdRatioBased(
                config.sample_ratio,
            ))))
            .with_resource(resource.clone())
            .build();

        let metrics = MetricExporter::builder()
            .with_http()
            .with_http_client(client()?)
            .with_protocol(Protocol::HttpBinary)
            .with_endpoint(config.signal_url("metrics"))
            .with_headers(headers)
            .with_timeout(EXPORT_TIMEOUT)
            .build()
            .map_err(OtlpError::Exporter)?;
        let meter_provider = SdkMeterProvider::builder()
            .with_periodic_exporter(metrics)
            .with_resource(resource)
            .build();

        Ok(Self {
            tracer_provider,
            meter_provider,
        })
    }

    /// Exports what is still queued and stops the export threads. Failures are logged, not
    /// returned: the process is ending either way.
    pub fn shutdown(self) {
        if let Err(err) = self.tracer_provider.shutdown() {
            tracing::warn!(error = %err, "flushing traces failed");
        }
        if let Err(err) = self.meter_provider.shutdown() {
            tracing::warn!(error = %err, "flushing metrics failed");
        }
    }
}

fn client() -> Result<reqwest::blocking::Client, OtlpError> {
    reqwest::blocking::Client::builder()
        .tls_backend_preconfigured(tls_config())
        .timeout(EXPORT_TIMEOUT)
        .user_agent(USER_AGENT)
        .build()
        .map_err(OtlpError::Client)
}
