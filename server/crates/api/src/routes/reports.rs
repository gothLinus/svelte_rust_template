//! `POST /csp-reports`: Content Security Policy violations, sent by browsers for the `report-uri`
//! and `report-to` directives (see `middleware::security_headers`).
//!
//! Outside the versioned API: browsers send reports without the CSRF header and as JSON,
//! `application/csp-report` for `report-uri` and `application/reports+json` for `report-to`.
//! Reports are logged, never stored. Anyone can send one, so every field is cut short and printed
//! escaped.

use application::Adapters;
use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, State},
    http::StatusCode,
    routing::post,
};
use serde_json::Value;

use crate::{extract::Client, problem::ApiError, rate_limit::Action, state::AppState};

const MAX_REPORT_BYTES: usize = 16 * 1024;
const MAX_FIELD_CHARS: usize = 256;
const MAX_REPORTS: usize = 10;

pub fn routes<A: Adapters>() -> Router<AppState<A>> {
    Router::new()
        .route("/csp-reports", post(receive::<A>))
        .layer(DefaultBodyLimit::max(MAX_REPORT_BYTES))
}

async fn receive<A: Adapters>(
    State(state): State<AppState<A>>,
    client: Client,
    body: Bytes,
) -> Result<StatusCode, ApiError> {
    state.limits.check_ip(Action::Report, client.ip()).await?;
    if let Ok(json) = serde_json::from_slice::<Value>(&body) {
        for report in violations(&json).take(MAX_REPORTS) {
            log(report);
        }
    }
    Ok(StatusCode::NO_CONTENT)
}

/// The violations in either format: a `{"csp-report": {…}}` object, or a Reporting API array of
/// `{"type": "csp-violation", "body": {…}}`.
fn violations(json: &Value) -> Box<dyn Iterator<Item = &Value> + '_> {
    match json {
        Value::Array(reports) => Box::new(
            reports
                .iter()
                .filter(|report| report["type"] == "csp-violation")
                .map(|report| &report["body"]),
        ),
        Value::Object(_) => Box::new(json.get("csp-report").into_iter()),
        _ => Box::new(std::iter::empty()),
    }
}

fn log(report: &Value) {
    tracing::warn!(
        directive = field(
            report,
            &[
                "effectiveDirective",
                "effective-directive",
                "violated-directive"
            ]
        ),
        blocked = field(report, &["blockedURL", "blocked-uri"]),
        document = field(report, &["documentURL", "document-uri"]),
        source = field(report, &["sourceFile", "source-file"]),
        line = field(report, &["lineNumber", "line-number"]),
        sample = field(report, &["sample", "script-sample"]),
        disposition = field(report, &["disposition"]),
        "content security policy violation"
    );
}

fn field(report: &Value, names: &[&str]) -> String {
    let Some(value) = names.iter().find_map(|name| report.get(*name)) else {
        return String::new();
    };
    let text = match value {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    };
    text.chars()
        .take(MAX_FIELD_CHARS)
        .flat_map(char::escape_default)
        .collect()
}
