//! `OTEL_*`: export of traces and metrics over OTLP/HTTP. Off unless
//! `OTEL_EXPORTER_OTLP_ENDPOINT` is set; logs go to stderr either way.
//!
//! The variables follow the OpenTelemetry specification, so a collector's documentation applies
//! as written: the endpoint is the collector's base URL (`/v1/traces` and `/v1/metrics` are
//! appended), headers are `key=value` pairs separated by commas, and the sampler argument is the
//! ratio of traces kept that do not continue a sampled one. Other `OTEL_*` variables are left to
//! the SDK, which reads the standard ones itself (`OTEL_RESOURCE_ATTRIBUTES`,
//! `OTEL_METRIC_EXPORT_INTERVAL`), so they are not reported as unknown.

use domain::secret::Secret;
use url::Url;

use super::reader::Reader;

#[derive(Debug, Clone, Default)]
pub struct TelemetryConfig {
    /// `None` keeps traces and metrics in the process.
    pub otlp: Option<OtlpConfig>,
}

#[derive(Debug, Clone)]
pub struct OtlpConfig {
    /// The collector's base URL, such as `http://localhost:4318`.
    pub endpoint: Url,
    /// Sent with every export, usually an API key; the values are secrets.
    pub headers: Vec<(String, Secret)>,
    /// `OTEL_SERVICE_NAME`, by default `APP_NAME`.
    pub service_name: String,
    /// The share of new traces kept, from 0 to 1. A trace the caller sampled is always kept.
    pub sample_ratio: f64,
}

impl OtlpConfig {
    /// The URL of one signal's endpoint, such as `traces`: the base URL with `/v1/<signal>`.
    pub fn signal_url(&self, signal: &str) -> String {
        format!(
            "{}/v1/{signal}",
            self.endpoint.as_str().trim_end_matches('/')
        )
    }
}

impl Reader<'_> {
    pub(super) fn telemetry(&mut self, app_name: &str) -> TelemetryConfig {
        let endpoint = self.optional("OTEL_EXPORTER_OTLP_ENDPOINT", parse_endpoint);
        let headers = self
            .optional("OTEL_EXPORTER_OTLP_HEADERS", parse_headers)
            .unwrap_or_default();
        let service_name = self
            .raw("OTEL_SERVICE_NAME")
            .unwrap_or_else(|| app_name.to_owned());
        let sample_ratio = self.parse("OTEL_TRACES_SAMPLER_ARG", 1.0, |raw| {
            raw.parse::<f64>()
                .ok()
                .filter(|ratio| (0.0..=1.0).contains(ratio))
                .ok_or_else(|| "must be a number from 0 to 1".to_owned())
        });
        if endpoint.is_none() && !headers.is_empty() {
            self.problem(
                "OTEL_EXPORTER_OTLP_HEADERS",
                "is set but OTEL_EXPORTER_OTLP_ENDPOINT is not",
            );
        }
        TelemetryConfig {
            otlp: endpoint.map(|endpoint| OtlpConfig {
                endpoint,
                headers,
                service_name,
                sample_ratio,
            }),
        }
    }
}

fn parse_endpoint(raw: &str) -> Result<Url, String> {
    let url =
        Url::parse(raw).map_err(|_| "must be a URL such as http://localhost:4318".to_owned())?;
    match url.scheme() {
        "http" | "https" if url.host().is_some() && url.query().is_none() => Ok(url),
        _ => Err("must be an http or https URL without a query".to_owned()),
    }
}

/// `key=value,key2=value2`, values percent-decoded as the specification asks.
fn parse_headers(raw: &str) -> Result<Vec<(String, Secret)>, String> {
    raw.split(',')
        .map(str::trim)
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (key, value) = pair
                .split_once('=')
                .ok_or_else(|| "must be key=value pairs separated by commas".to_owned())?;
            let key = key.trim();
            let valid_key = !key.is_empty()
                && key
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte));
            if !valid_key {
                return Err(format!("`{key}` is not a header name"));
            }
            let value = percent_decode(value.trim())
                .ok_or_else(|| format!("the value of `{key}` is not valid percent-encoding"))?;
            Ok((key.to_ascii_lowercase(), Secret::new(value)))
        })
        .collect()
}

fn percent_decode(raw: &str) -> Option<String> {
    let bytes = raw.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' {
            let hex = raw.get(index + 1..index + 3)?;
            decoded.push(u8::from_str_radix(hex, 16).ok()?);
            index += 3;
        } else {
            decoded.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8(decoded).ok()
}
