//! Logs, traces and metrics.
//!
//! Logs always go to stderr, as text or JSON (`LOG_FORMAT`), filtered by `RUST_LOG`. With
//! `OTEL_EXPORTER_OTLP_ENDPOINT` set, [`init`] also exports the same spans as traces and the
//! request metrics (`middleware::metrics`) to an OpenTelemetry collector, continues traces a caller
//! started (W3C `traceparent`, see [`continue_trace`]), and problem documents name their trace
//! ([`current_trace_id`]). Without it the OpenTelemetry API stays a no-op.

use axum::http::HeaderMap;
use infrastructure::{
    config::{LogFormat, OtlpConfig},
    telemetry::Otlp,
};
use opentelemetry::{
    global,
    propagation::Extractor,
    trace::{TraceContextExt, TracerProvider},
};
use opentelemetry_sdk::propagation::TraceContextPropagator;
use tracing::Span;
use tracing_opentelemetry::{OpenTelemetrySpanExt, layer as otel_layer};
use tracing_subscriber::{EnvFilter, Layer, fmt, layer::SubscriberExt, util::SubscriberInitExt};

pub const DEFAULT_FILTER: &str =
    "info,api=debug,application=debug,infrastructure=info,domain=info,tower_http=info,sqlx=warn";

/// The instrumentation scope of the server's spans and metrics.
pub const SCOPE: &str = "api";

pub type TelemetryError = Box<dyn std::error::Error + Send + Sync>;

/// What [`init`] started; [`Telemetry::shutdown`] flushes it before the process exits.
#[must_use = "dropping it without `shutdown` loses the spans and metrics still queued"]
pub struct Telemetry {
    otlp: Option<Otlp>,
}

impl Telemetry {
    pub fn shutdown(self) {
        if let Some(otlp) = self.otlp {
            otlp.shutdown();
        }
    }
}

/// Installs the global subscriber, and with `otlp` the exporters. Call it once, before the async
/// runtime starts (see [`Otlp::start`]).
pub fn init(format: LogFormat, otlp: Option<&OtlpConfig>) -> Result<Telemetry, TelemetryError> {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER));
    let logs = match format {
        LogFormat::Text => fmt::layer().with_writer(std::io::stderr).boxed(),
        LogFormat::Json => fmt::layer()
            .json()
            .flatten_event(true)
            .with_writer(std::io::stderr)
            .boxed(),
    };

    let otlp = otlp.map(Otlp::start).transpose()?;
    let traces = otlp
        .as_ref()
        .map(|otlp| otel_layer().with_tracer(otlp.tracer_provider.tracer(SCOPE)));
    if let Some(otlp) = &otlp {
        global::set_text_map_propagator(TraceContextPropagator::new());
        global::set_meter_provider(otlp.meter_provider.clone());
    }

    tracing_subscriber::registry()
        .with(filter)
        .with(logs)
        .with(traces)
        .try_init()?;
    Ok(Telemetry { otlp })
}

/// Makes `span` part of the trace the caller started, if its headers carry a W3C `traceparent`.
/// A no-op unless traces are exported.
pub fn continue_trace(span: &Span, headers: &HeaderMap) {
    let parent =
        global::get_text_map_propagator(|propagator| propagator.extract(&HeaderExtractor(headers)));
    if parent.span().span_context().is_valid() {
        // Fails only without the OpenTelemetry layer, where there is nothing to continue.
        let _ = span.set_parent(parent);
    }
}

/// The id of the trace the current span belongs to, while traces are exported.
pub fn current_trace_id() -> Option<String> {
    let context = Span::current().context();
    let span = context.span();
    let span_context = span.span_context();
    span_context
        .is_valid()
        .then(|| span_context.trace_id().to_string())
}

struct HeaderExtractor<'a>(&'a HeaderMap);

impl Extractor for HeaderExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|value| value.to_str().ok())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(axum::http::HeaderName::as_str).collect()
    }
}
