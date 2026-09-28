//! The API's wire format: Protocol Buffers messages (the `proto` crate, generated from `/proto`)
//! as `application/x-protobuf`, in both directions. Errors stay `application/problem+json` (see
//! `problem`), and a few endpoints are JSON on purpose (health checks, the personal data export).
//!
//! The application layer knows nothing of messages: its DTOs use `Uuid`, `OffsetDateTime` and
//! `SecretInput`. This module is the seam. A handler takes and returns DTOs wrapped in [`Proto`],
//! and one file per feature here converts them:
//!
//! - [`IntoMessage`] turns a response DTO into its message;
//! - [`FromMessage`] turns a decoded request message into a request DTO, rejecting values the DTO
//!   cannot hold;
//! - the `page_message!` macro covers a keyset-paginated list in one line.
//!
//! [`Proto`] answers a body that is not a protobuf request with `415`, one that does not decode
//! with `400 invalid_body`, and one that decodes but holds an unusable value (a malformed id, an
//! unset enum) with a `422` naming the field. There is no content negotiation: requests must be
//! protobuf, and so are successful responses.
//!
//! To add a resource: define its messages in `/proto/api/v1/`, add a file here with the
//! conversions (`notes.rs` is the reference; declare it with a `mod` line below), and run
//! `just gen-types` for the frontend.

use application::{AppError, ValidationErrors};
use axum::{
    body::Bytes,
    extract::{FromRequest, Request},
    http::{HeaderMap, HeaderValue, StatusCode, header::CONTENT_TYPE},
    response::{IntoResponse, Response},
};
use domain::{error::ValidationError, i18n};
use proto::{Message, Timestamp};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::problem::ApiError;

mod account;
mod admin;
mod auth;
mod common;
mod mfa;
mod notes;
mod oauth;
mod passkeys;
mod passwordless;

pub use common::permission as permission_message;

pub const PROTOBUF: &str = "application/x-protobuf";

/// Converts a response DTO into the message sent as the body; implement it for every DTO a
/// handler returns as [`Proto<T>`](Proto).
///
/// The conversion cannot fail: whatever a DTO holds fits its message. Ids are sent as strings
/// and timestamps as [`Timestamp`]. A `Vec<Dto>` returned as a bare list needs its own impl
/// onto a list message, since Protocol Buffers has no top-level array.
///
/// # Examples
///
/// ```
/// use api::wire::IntoMessage;
/// use proto::v1;
///
/// struct Titled {
///     title: String,
/// }
///
/// impl IntoMessage for Titled {
///     type Message = v1::Note;
///
///     fn into_message(self) -> v1::Note {
///         v1::Note {
///             title: self.title,
///             ..Default::default()
///         }
///     }
/// }
///
/// let message = Titled { title: "Hello".into() }.into_message();
/// assert_eq!(message.title, "Hello");
/// ```
pub trait IntoMessage {
    type Message: Message;

    fn into_message(self) -> Self::Message;
}

/// Implements [`IntoMessage`] for a page of `$item`s as `$page`, a message with
/// `repeated $item items = 1; optional string next_cursor = 2;`. Protocol Buffers has no
/// generics, so each resource declares its own page message and one line here, next to the
/// item's own conversion.
///
/// The macro is crate-private, so a doctest cannot compile this example (hence `ignore`):
///
/// ```ignore
/// page_message!(NoteDto => v1::NotePage);
/// ```
macro_rules! page_message {
    ($item:ty => $page:path) => {
        impl $crate::wire::IntoMessage for ::application::pagination::PageDto<$item> {
            type Message = $page;

            fn into_message(self) -> $page {
                $page {
                    items: self
                        .items
                        .into_iter()
                        .map($crate::wire::IntoMessage::into_message)
                        .collect(),
                    next_cursor: self.next_cursor,
                }
            }
        }
    };
}
pub(crate) use page_message;

/// Converts a decoded request message into a request DTO; implement it for every DTO a
/// handler takes as [`Proto<T>`](Proto).
///
/// Protocol Buffers cannot tell an absent scalar from its zero value, so DTO fields that
/// distinguish them (a partial update) come from `optional` message fields. The conversion
/// carries values across and rejects what the DTO cannot represent, such as a malformed id or
/// an unset enum. Rules about the values themselves (lengths, formats) belong to the domain's
/// value objects, which the service applies.
///
/// # Examples
///
/// ```
/// use api::wire::FromMessage;
/// use application::ValidationErrors;
/// use domain::error::ValidationError;
/// use proto::v1;
///
/// struct Rename {
///     title: String,
/// }
///
/// impl FromMessage for Rename {
///     type Message = v1::UpdateNoteRequest;
///
///     fn from_message(message: v1::UpdateNoteRequest) -> Result<Self, ValidationErrors> {
///         let title = message
///             .title
///             .ok_or_else(|| ValidationErrors::single("title", &ValidationError::required()))?;
///         Ok(Self { title })
///     }
/// }
///
/// let missing = Rename::from_message(v1::UpdateNoteRequest::default());
/// assert!(missing.is_err());
/// ```
pub trait FromMessage: Sized {
    type Message: Message + Default;

    fn from_message(message: Self::Message) -> Result<Self, ValidationErrors>;
}

/// A Protocol Buffers body: extracts a [`FromMessage`] DTO from the request and sends an
/// [`IntoMessage`] one as the response (`200` unless the handler pairs it with a status).
///
/// As an extractor it rejects a request without `Content-Type: application/x-protobuf`
/// (parameters allowed) with `415 unsupported_media_type`, an unreadable or oversized body with
/// the status axum reports (`413` past the body limit), an undecodable message with
/// `400 invalid_body`, and a [`FromMessage`] failure with `422`. It consumes the body, so it must
/// be the last extractor of a handler.
#[derive(Debug)]
pub struct Proto<T>(pub T);

impl<T: IntoMessage> IntoResponse for Proto<T> {
    fn into_response(self) -> Response {
        let body = self.0.into_message().encode_to_vec();
        ([(CONTENT_TYPE, HeaderValue::from_static(PROTOBUF))], body).into_response()
    }
}

impl<S: Send + Sync, T: FromMessage> FromRequest<S> for Proto<T> {
    type Rejection = ApiError;

    async fn from_request(request: Request, state: &S) -> Result<Self, ApiError> {
        if !is_protobuf(request.headers()) {
            return Err(ApiError::new(
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                "unsupported_media_type",
                i18n::Message::new("http-unsupported-media-type").arg("type", PROTOBUF),
            ));
        }
        // Buffered within `DefaultBodyLimit`, like every other body.
        let body = Bytes::from_request(request, state)
            .await
            .map_err(|rejection| {
                tracing::debug!(reason = %rejection.body_text(), "request body rejected");
                let detail = if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE {
                    i18n::Message::new("http-payload-too-large")
                } else {
                    i18n::Message::new("http-invalid-request")
                };
                ApiError::new(rejection.status(), "invalid_request", detail)
            })?;
        let message = T::Message::decode(body).map_err(|err| {
            tracing::debug!(error = %err, "protobuf body rejected");
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_body",
                i18n::Message::new("http-invalid-body"),
            )
        })?;
        T::from_message(message)
            .map(Proto)
            .map_err(|errors| AppError::Validation(errors).into())
    }
}

fn is_protobuf(headers: &HeaderMap) -> bool {
    headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|mime| mime.trim().eq_ignore_ascii_case(PROTOBUF))
}

fn timestamp(at: OffsetDateTime) -> Timestamp {
    Timestamp {
        seconds: at.unix_timestamp(),
        // Always below 10^9.
        nanos: i32::try_from(at.nanosecond()).unwrap_or_default(),
    }
}

fn uuid(field: &str, value: &str) -> Result<Uuid, ValidationErrors> {
    if value.is_empty() {
        return Err(ValidationErrors::single(
            field,
            &ValidationError::required(),
        ));
    }
    Uuid::try_parse(value).map_err(|_| {
        ValidationErrors::single(
            field,
            &ValidationError::new("invalid", i18n::Message::new("validation-invalid-uuid")),
        )
    })
}

fn invalid_enum(field: &str, value: i32) -> ValidationErrors {
    let error = if value == 0 {
        ValidationError::required()
    } else {
        ValidationError::new(
            "invalid",
            i18n::Message::new("validation-not-allowed-value"),
        )
    };
    ValidationErrors::single(field, &error)
}
