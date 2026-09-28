//! The single error type of the HTTP layer, rendered as RFC 9457 Problem Details.
//!
//! Every failure (a use case error, a malformed body, a rate limit, a timeout, a panic) becomes an
//! [`ApiError`] and leaves as `application/problem+json` with a stable `code`. Server errors are
//! logged with their full cause and never leak it to the client.
//!
//! The words of a problem document (`title`, `detail`, each field's `message`) are catalog
//! messages, written in the reader's language. An [`ApiError`] therefore does not render itself:
//! [`IntoResponse`] leaves the [`ApiError`] in the response's extensions behind an empty body, and
//! the `localize` middleware, which sits outside everything that can answer with one (extractors,
//! layers, timeouts, panics, fallbacks), renders the body with the request's `Accept-Language`.
//! That keeps a single place responsible for the language, however the error came about.

use std::time::Duration;

use application::{AppError, FieldError};
use axum::{
    body::Body,
    extract::rejection::{FormRejection, PathRejection, QueryRejection},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use domain::{
    error::ErrorChain,
    i18n::{Locale, Message, Translator},
};
use serde::Serialize;

pub const PROBLEM_JSON: &str = "application/problem+json";

/// The error body of every failed request: an `application/problem+json` document
/// ([RFC 9457](https://www.rfc-editor.org/rfc/rfc9457)).
///
/// Errors stay JSON while successful responses are Protocol Buffers: RFC 9457 defines the format,
/// and proxies, curl and logs can read it. `type`, `title`, `status`, `detail` and `instance` are
/// the standard members. `code` is a stable machine-readable identifier (switch on it, not on
/// `title`), and `errors` lists invalid fields for `422` responses. The `x-request-id` response
/// header identifies the request in the logs.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProblemDetails {
    #[serde(rename = "type")]
    pub kind: String,
    pub title: String,
    pub status: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
    pub code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<Vec<ProblemField>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProblemField {
    pub field: String,
    pub code: String,
    pub message: String,
}

impl ProblemField {
    fn render(error: FieldError, translator: &dyn Translator, locale: &Locale) -> Self {
        Self {
            field: error.field,
            code: error.code,
            message: translator.translate(locale, &error.message),
        }
    }
}

/// How long a client waits before retrying work the server was too busy for. Hashing a password
/// takes tens of milliseconds, so the queue drains well within it.
const BUSY_RETRY_AFTER: Duration = Duration::from_secs(1);

/// A failed request, ready to become a problem document. Handlers return it as the error of
/// `Result<_, ApiError>`, usually by `?` on an [`AppError`]; the constructors cover failures that
/// arise in the HTTP layer itself.
///
/// The response is `application/problem+json`; `429` and a busy `503` also carry `Retry-After` in
/// whole seconds. Server errors are logged with their cause and answered with a generic message.
#[derive(Debug, Clone)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    detail: Message,
    errors: Option<Vec<FieldError>>,
    retry_after: Option<Duration>,
}

impl ApiError {
    pub fn new(status: StatusCode, code: &'static str, detail: Message) -> Self {
        Self {
            status,
            code,
            detail,
            errors: None,
            retry_after: None,
        }
    }

    pub fn status(&self) -> StatusCode {
        self.status
    }

    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn not_found() -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            "not_found",
            Message::new("error-not-found"),
        )
    }

    pub fn method_not_allowed() -> Self {
        Self::new(
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
            Message::new("http-method-not-allowed"),
        )
    }

    pub fn unauthenticated() -> Self {
        AppError::Unauthenticated.into()
    }

    pub fn forbidden() -> Self {
        AppError::Forbidden.into()
    }

    pub fn csrf_rejected(detail: Message) -> Self {
        Self::new(StatusCode::FORBIDDEN, "csrf_rejected", detail)
    }

    pub fn rate_limited(retry_after: Duration) -> Self {
        Self {
            retry_after: Some(retry_after),
            ..Self::new(
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                Message::new("http-rate-limited"),
            )
        }
    }

    pub fn timeout() -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "timeout",
            Message::new("http-timeout"),
        )
    }

    pub fn unavailable(detail: Message) -> Self {
        Self::new(StatusCode::SERVICE_UNAVAILABLE, "unavailable", detail)
    }

    pub fn internal(err: &(dyn std::error::Error + 'static)) -> Self {
        tracing::error!(error = %ErrorChain(err), "request failed");
        Self::internal_without_cause()
    }

    /// Logs the failure of work no response carries (see `Background`): server errors with their
    /// cause, like any request's, and anything else at debug level.
    pub fn log_detached(err: AppError) {
        if matches!(err, AppError::Internal(_)) {
            let _logged = Self::from(err);
        } else {
            tracing::debug!(code = err.code(), "background work refused");
        }
    }

    pub fn internal_without_cause() -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            Message::new("error-internal"),
        )
    }

    fn body(self, translator: &dyn Translator, locale: &Locale) -> ProblemDetails {
        ProblemDetails {
            kind: "about:blank".to_owned(),
            title: translator.translate(locale, &title(self.status)),
            status: self.status.as_u16(),
            detail: Some(translator.translate(locale, &self.detail)),
            instance: None,
            code: self.code.to_owned(),
            errors: self.errors.map(|errors| {
                errors
                    .into_iter()
                    .map(|error| ProblemField::render(error, translator, locale))
                    .collect()
            }),
        }
    }

    /// Fills the placeholder body [`IntoResponse`] left with the problem document in `locale`, and
    /// says which language it is in.
    pub(crate) fn render(
        self,
        mut response: Response,
        translator: &dyn Translator,
        locale: &Locale,
    ) -> Response {
        let body = serde_json::to_vec(&self.body(translator, locale)).unwrap_or_default();
        let headers = response.headers_mut();
        headers.remove(header::CONTENT_LENGTH);
        if let Ok(language) = HeaderValue::from_str(locale.as_str()) {
            headers.insert(header::CONTENT_LANGUAGE, language);
        }
        headers.append(header::VARY, HeaderValue::from_static("accept-language"));
        *response.body_mut() = Body::from(body);
        response
    }
}

fn title(status: StatusCode) -> Message {
    Message::new(match status.as_u16() {
        400 => "http-status-400",
        401 => "http-status-401",
        403 => "http-status-403",
        404 => "http-status-404",
        405 => "http-status-405",
        409 => "http-status-409",
        413 => "http-status-413",
        415 => "http-status-415",
        422 => "http-status-422",
        429 => "http-status-429",
        500 => "http-status-500",
        502 => "http-status-502",
        503 => "http-status-503",
        _ => "http-status-other",
    })
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let mut response = self.status.into_response();
        let headers = response.headers_mut();
        headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(PROBLEM_JSON));
        if let Some(retry_after) = self.retry_after {
            // Whole seconds, rounded up so a client that waits exactly this long succeeds.
            let seconds = retry_after.as_secs() + u64::from(retry_after.subsec_nanos() > 0);
            headers.insert(header::RETRY_AFTER, HeaderValue::from(seconds.max(1)));
        }
        response.extensions_mut().insert(self);
        response
    }
}

/// Maps each [`AppError`] to a status and its stable [`AppError::code`]: `422` for validation (with
/// the invalid fields), `401` for missing or wrong credentials, `403` for refused access
/// (including an unverified email, a disabled account and a required re-authentication), `404`,
/// `409`, `400` for a bad token or passkey, `502` when a social
/// provider is down, `503` (with `Retry-After`) when the server is busy, and `500` for everything
/// internal.
impl From<AppError> for ApiError {
    fn from(err: AppError) -> Self {
        let code = err.code();
        let status = match &err {
            AppError::Validation(_) => StatusCode::UNPROCESSABLE_ENTITY,
            AppError::Unauthenticated | AppError::InvalidCredentials => StatusCode::UNAUTHORIZED,
            AppError::EmailNotVerified
            | AppError::AccountDisabled
            | AppError::Forbidden
            | AppError::ReauthRequired => StatusCode::FORBIDDEN,
            AppError::NotFound => StatusCode::NOT_FOUND,
            AppError::Conflict { .. } => StatusCode::CONFLICT,
            AppError::InvalidToken | AppError::InvalidPasskey => StatusCode::BAD_REQUEST,
            AppError::ProviderUnavailable => StatusCode::BAD_GATEWAY,
            AppError::Busy => StatusCode::SERVICE_UNAVAILABLE,
            AppError::Internal(source) => return Self::internal(source),
        };
        let detail = err.message();
        let retry_after = matches!(err, AppError::Busy).then_some(BUSY_RETRY_AFTER);

        let errors = match err {
            AppError::Validation(errors) => Some(errors.into_fields()),
            _ => None,
        };
        Self {
            errors,
            retry_after,
            ..Self::new(status, code, detail)
        }
    }
}

impl From<FormRejection> for ApiError {
    fn from(rejection: FormRejection) -> Self {
        tracing::debug!(reason = %rejection.body_text(), "form rejected");
        Self::new(
            rejection.status(),
            "invalid_body",
            Message::new("http-invalid-body"),
        )
    }
}

impl From<QueryRejection> for ApiError {
    fn from(rejection: QueryRejection) -> Self {
        tracing::debug!(reason = %rejection.body_text(), "query string rejected");
        Self::new(
            StatusCode::BAD_REQUEST,
            "invalid_query",
            Message::new("http-invalid-query"),
        )
    }
}

impl From<PathRejection> for ApiError {
    fn from(rejection: PathRejection) -> Self {
        tracing::debug!(reason = %rejection.body_text(), "path rejected");
        // A malformed id cannot name an existing resource.
        Self::not_found()
    }
}
