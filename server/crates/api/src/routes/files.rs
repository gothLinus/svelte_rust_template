//! `/api/v1/files`: files a user uploaded. The list, reading, renaming and deleting work like
//! `routes/notes.rs`; uploads and downloads carry the contents as raw bytes, streamed between the
//! client and the object store as they arrive, so no file ever sits in the server's memory whole.
//!
//! - `POST /files?name=<name>`: the body is the file, with its `Content-Type` and a
//!   `Content-Length` (`411` without one: the store needs the length up front, and the size and
//!   quota are checked before a byte is read). Answers `201` with the file. Its own timeout
//!   (`UPLOAD_TIMEOUT`, see [`is_upload`]) and rate limits (`Action::Upload`) apply instead of the
//!   API's.
//! - `GET /files/{id}/content`: the file, always as an attachment: the browser saves it rather
//!   than showing it under the app's origin, where an uploaded web page could run scripts. With
//!   the API's `Content-Security-Policy: default-src 'none'` and `nosniff`, no upload can become
//!   content of the app.
//! - `GET /files/usage`: how much the user stores against their quota.
//!
//! The rest of the slice: `domain::file`, `application::files`,
//! `infrastructure::db::repositories::files` and `infrastructure::object_store`, `wire/files.rs`
//! and `proto/api/v1/files.proto`.

use std::fmt::Write as _;

use application::{
    Adapters,
    files::dto::{
        FileDownload, FileDto, FileUsageDto, ListFilesQuery, UpdateFileRequest, UploadFileRequest,
    },
    pagination::PageDto,
};
use axum::{
    Extension, Router,
    body::Body,
    extract::State,
    http::{
        HeaderMap, HeaderValue, Method, StatusCode,
        header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE, LOCATION},
    },
    response::{IntoResponse, Response},
    routing::get,
};
use domain::{
    file::{ContentType, FileId},
    i18n::Message,
    object_store::{ByteStream, ObjectStoreError},
};
use futures_util::TryStreamExt;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    extract::{Client, CurrentUser, Path, Proto, Query},
    problem::ApiError,
    rate_limit::Action,
    state::AppState,
};

/// Marks a response whose body is a file's contents, which the router sends as they are: never
/// compressed on the way (see `router::NotFileContents`).
#[derive(Debug, Clone, Copy)]
pub(crate) struct FileContents;

const UPLOAD_PATH: &str = "/api/v1/files";

pub fn routes<A: Adapters>() -> Router<AppState<A>> {
    Router::new()
        .route("/files", get(list::<A>).post(upload::<A>))
        .route("/files/usage", get(usage::<A>))
        .route(
            "/files/{id}",
            get(show::<A>).patch(update::<A>).delete(destroy::<A>),
        )
        .route("/files/{id}/content", get(download::<A>))
}

/// Whether a request is an upload, which may take `UPLOAD_TIMEOUT` rather than `REQUEST_TIMEOUT`:
/// the router's timeout is applied outside all routes, before one is chosen, so it asks this.
pub fn is_upload(method: &Method, path: &str) -> bool {
    method == Method::POST && path.trim_end_matches('/') == UPLOAD_PATH
}

async fn list<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
    Query(query): Query<ListFilesQuery>,
) -> Result<Proto<PageDto<FileDto>>, ApiError> {
    Ok(Proto(state.services.files.list(user.actor(), query).await?))
}

async fn usage<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
) -> Result<Proto<FileUsageDto>, ApiError> {
    Ok(Proto(state.services.files.usage(user.actor()).await?))
}

async fn show<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<Proto<FileDto>, ApiError> {
    Ok(Proto(
        state
            .services
            .files
            .get(user.actor(), FileId::from_uuid(id))
            .await?,
    ))
}

#[derive(Debug, Deserialize)]
struct UploadQuery {
    #[serde(default)]
    name: String,
}

async fn upload<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
    client: Client,
    Query(query): Query<UploadQuery>,
    headers: HeaderMap,
    body: Body,
) -> Result<Response, ApiError> {
    state.limits.check_ip(Action::Upload, client.ip()).await?;
    state
        .limits
        .check_user(Action::Upload, user.actor().user_id)
        .await?;

    let request = UploadFileRequest {
        name: query.name,
        content_type: headers
            .get(CONTENT_TYPE)
            .map(|value| String::from_utf8_lossy(value.as_bytes()).into_owned())
            .unwrap_or_default(),
        size: content_length(&headers)?,
    };
    let body: ByteStream = Box::pin(body.into_data_stream().map_err(ObjectStoreError::body));
    let file = state
        .services
        .files
        .upload(user.actor(), request, body)
        .await?;

    let location = HeaderValue::from_str(&format!("{UPLOAD_PATH}/{}", file.id))
        .map_err(|err| ApiError::internal(&err))?;
    Ok((StatusCode::CREATED, [(LOCATION, location)], Proto(file)).into_response())
}

fn content_length(headers: &HeaderMap) -> Result<u64, ApiError> {
    let value = headers.get(CONTENT_LENGTH).ok_or_else(|| {
        ApiError::new(
            StatusCode::LENGTH_REQUIRED,
            "length_required",
            Message::new("http-length-required"),
        )
    })?;
    value
        .to_str()
        .ok()
        .and_then(|value| value.trim().parse().ok())
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_request",
                Message::new("http-invalid-request"),
            )
        })
}

async fn download<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<Response, ApiError> {
    let FileDownload {
        name,
        content_type,
        size,
        body,
    } = state
        .services
        .files
        .download(user.actor(), FileId::from_uuid(id))
        .await?;

    let content_type = HeaderValue::from_str(&content_type)
        .unwrap_or_else(|_| HeaderValue::from_static(ContentType::OCTET_STREAM));
    Ok((
        Extension(FileContents),
        [
            (CONTENT_TYPE, content_type),
            (CONTENT_LENGTH, HeaderValue::from(size)),
            (CONTENT_DISPOSITION, attachment(&name)),
        ],
        Body::from_stream(body),
    )
        .into_response())
}

/// `Content-Disposition: attachment` naming the file: `filename` for old clients, with what is not
/// printable ASCII (and quotes and backslashes) replaced, and `filename*` with the name as it is,
/// percent-encoded as UTF-8 (RFC 6266 and RFC 8187).
fn attachment(name: &str) -> HeaderValue {
    let fallback: String = name
        .chars()
        .map(|c| {
            if c == ' ' || (c.is_ascii_graphic() && c != '"' && c != '\\') {
                c
            } else {
                '_'
            }
        })
        .collect();
    let encoded: String = name.bytes().fold(String::new(), |mut out, byte| {
        if byte.is_ascii_alphanumeric() || b"!#$&+-.^_`|~".contains(&byte) {
            out.push(char::from(byte));
        } else {
            let _infallible = write!(out, "%{byte:02X}");
        }
        out
    });
    HeaderValue::from_str(&format!(
        "attachment; filename=\"{fallback}\"; filename*=UTF-8''{encoded}"
    ))
    .unwrap_or_else(|_| HeaderValue::from_static("attachment"))
}

async fn update<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    Proto(body): Proto<UpdateFileRequest>,
) -> Result<Proto<FileDto>, ApiError> {
    Ok(Proto(
        state
            .services
            .files
            .update(user.actor(), FileId::from_uuid(id), body)
            .await?,
    ))
}

async fn destroy<A: Adapters>(
    State(state): State<AppState<A>>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    state
        .services
        .files
        .delete(user.actor(), FileId::from_uuid(id))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
