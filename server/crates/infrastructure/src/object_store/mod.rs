//! The object store adapter: `domain::object_store::ObjectStore` over the S3 API, which RustFS
//! (the development store in `compose.yaml`), MinIO, Ceph, Garage and AWS S3 speak alike.
//!
//! Three requests do the work, all path-style (`<endpoint>/<bucket>/<key>`) and signed with AWS
//! Signature Version 4 ([`signing`]): `PUT` streams an object in with its length up front, `GET`
//! streams one out, `DELETE` removes one. [`S3ObjectStore::ensure_bucket`] runs once at startup and
//! creates the bucket if it is missing, so a wrong endpoint or key stops the server there instead
//! of failing the first upload.
//!
//! A hand-written client rather than an SDK: these calls are all the application needs, and they
//! reuse the HTTP stack (reqwest, rustls on ring) the other adapters already bring.

use std::{
    pin::Pin,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    task::{Context, Poll, ready},
    time::Duration,
};

use bytes::Bytes;
use domain::object_store::{
    ByteStream, NewObject, Object, ObjectKey, ObjectStore, ObjectStoreError,
};
use futures_core::Stream;
use futures_util::TryStreamExt;
use reqwest::{
    Method, StatusCode,
    header::{AUTHORIZATION, CONTENT_LENGTH, CONTENT_TYPE, HeaderValue},
};
use thiserror::Error;
use time::OffsetDateTime;
use url::Url;

use crate::config::StorageConfig;

pub use signing::Signer;

pub mod signing;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const READ_TIMEOUT: Duration = Duration::from_secs(30);

/// The store answered with an error status. `code` is S3's error code (`AccessDenied`,
/// `NoSuchBucket`), when its XML body had one.
#[derive(Debug, Error)]
#[error("the object store answered {status}{}", .code.as_deref().map(|code| format!(" ({code})")).unwrap_or_default())]
pub struct S3Error {
    pub status: StatusCode,
    pub code: Option<String>,
}

#[derive(Debug, Error)]
#[error("the body was {0} than its announced length")]
struct WrongLength(&'static str);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BucketStatus {
    Existed,
    Created,
}

#[derive(Debug, Clone)]
pub struct S3ObjectStore {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    http: reqwest::Client,
    bucket_url: Url,
    signer: Signer,
    region: String,
}

impl S3ObjectStore {
    /// A client for the bucket in `config`, over its own connection pool.
    ///
    /// # Errors
    ///
    /// Fails if the endpoint and bucket do not make a URL, or the HTTP client cannot be built.
    pub fn new(config: &StorageConfig) -> Result<Self, ObjectStoreError> {
        let bucket_url = Url::parse(&format!("{}/{}", config.endpoint, config.bucket))
            .map_err(ObjectStoreError::backend)?;
        let http = reqwest::Client::builder()
            .tls_backend_preconfigured(crate::oauth::tls_config())
            .connect_timeout(CONNECT_TIMEOUT)
            .read_timeout(READ_TIMEOUT)
            .user_agent(concat!("svelte-rust-template/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(ObjectStoreError::backend)?;
        Ok(Self {
            inner: Arc::new(Inner {
                http,
                bucket_url,
                signer: Signer {
                    access_key: config.access_key.clone(),
                    secret_key: config.secret_key.clone(),
                    region: config.region.clone(),
                    service: "s3".to_owned(),
                },
                region: config.region.clone(),
            }),
        })
    }

    /// Checks that the bucket exists and the credentials may use it, and creates it if it does not
    /// exist.
    ///
    /// # Errors
    ///
    /// Fails if the store cannot be reached, refuses the credentials, or will not create the
    /// bucket.
    pub async fn ensure_bucket(&self) -> Result<BucketStatus, ObjectStoreError> {
        let bucket = self.inner.bucket_url.clone();
        let response = self
            .inner
            .send(Method::HEAD, bucket.clone(), None, &[], None)
            .await?;
        match response.status() {
            status if status.is_success() => return Ok(BucketStatus::Existed),
            StatusCode::NOT_FOUND => {}
            status => {
                return Err(ObjectStoreError::backend(S3Error { status, code: None }));
            }
        }

        // Outside AWS's default region, S3 wants the region in the body.
        let body = (self.inner.region != "us-east-1").then(|| {
            Bytes::from(format!(
                "<CreateBucketConfiguration xmlns=\"http://s3.amazonaws.com/doc/2006-03-01/\">\
                 <LocationConstraint>{}</LocationConstraint></CreateBucketConfiguration>",
                self.inner.region
            ))
        });
        let response = self
            .inner
            .send(Method::PUT, bucket, None, &[], body)
            .await?;
        if response.status().is_success() {
            Ok(BucketStatus::Created)
        } else {
            Err(rejected(response).await)
        }
    }

    pub fn bucket_url(&self) -> &Url {
        &self.inner.bucket_url
    }
}

impl Inner {
    fn object_url(&self, key: &ObjectKey) -> Url {
        let mut url = self.bucket_url.clone();
        // Keys are made of URL-safe characters (see `ObjectKey`), so the path needs no escaping and
        // signs as it is sent.
        url.set_path(&format!("{}/{key}", self.bucket_url.path()));
        url
    }

    async fn send(
        &self,
        method: Method,
        url: Url,
        stream: Option<(u64, reqwest::Body)>,
        headers: &[(&str, &str)],
        body: Option<Bytes>,
    ) -> Result<reqwest::Response, ObjectStoreError> {
        let payload = match (&stream, &body) {
            (Some(_), _) => signing::UNSIGNED_PAYLOAD.to_owned(),
            (None, Some(body)) => signing::payload_digest(body),
            (None, None) => signing::EMPTY_PAYLOAD.to_owned(),
        };
        let signature = self.signer.sign(
            method.as_str(),
            &url,
            headers,
            &payload,
            OffsetDateTime::now_utc(),
        );

        let mut request = self
            .http
            .request(method, url)
            .header("x-amz-date", signature.amz_date)
            .header("x-amz-content-sha256", signature.content_sha256)
            .header(AUTHORIZATION, signature.authorization);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        request = match (stream, body) {
            // The length is sent up front: S3 does not take chunked bodies.
            (Some((length, stream)), _) => request
                .header(CONTENT_LENGTH, HeaderValue::from(length))
                .body(stream),
            (None, Some(body)) => request.body(body),
            (None, None) => request,
        };
        request.send().await.map_err(ObjectStoreError::backend)
    }
}

async fn rejected(response: reqwest::Response) -> ObjectStoreError {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    let code = body
        .split_once("<Code>")
        .and_then(|(_, rest)| rest.split_once("</Code>"))
        .map(|(code, _)| code.trim().to_owned());
    ObjectStoreError::backend(S3Error { status, code })
}

impl ObjectStore for S3ObjectStore {
    async fn put(&self, key: &ObjectKey, object: NewObject) -> Result<(), ObjectStoreError> {
        let failed = Arc::new(AtomicBool::new(false));
        let body = reqwest::Body::wrap_stream(Measured::new(
            object.body,
            object.length,
            Arc::clone(&failed),
        ));
        let result = self
            .inner
            .send(
                Method::PUT,
                self.inner.object_url(key),
                Some((object.length, body)),
                &[(CONTENT_TYPE.as_str(), &object.content_type)],
                None,
            )
            .await;

        // When the body failed, the store only saw a connection that broke: the reason is the
        // body's, and not the store's fault.
        if failed.load(Ordering::Acquire) {
            let source = match result {
                Err(ObjectStoreError::Backend(source) | ObjectStoreError::Body(source)) => source,
                Ok(_) => Box::new(WrongLength("longer")),
            };
            return Err(ObjectStoreError::Body(source));
        }
        let response = result?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(rejected(response).await)
        }
    }

    async fn get(&self, key: &ObjectKey) -> Result<Option<Object>, ObjectStoreError> {
        let response = self
            .inner
            .send(Method::GET, self.inner.object_url(key), None, &[], None)
            .await?;
        match response.status() {
            StatusCode::OK => {}
            StatusCode::NOT_FOUND => return Ok(None),
            _ => return Err(rejected(response).await),
        }
        let length = response.content_length().unwrap_or_default();
        let body: ByteStream = Box::pin(response.bytes_stream().map_err(ObjectStoreError::body));
        Ok(Some(Object { length, body }))
    }

    async fn delete(&self, key: &ObjectKey) -> Result<(), ObjectStoreError> {
        let response = self
            .inner
            .send(Method::DELETE, self.inner.object_url(key), None, &[], None)
            .await?;
        if response.status().is_success() || response.status() == StatusCode::NOT_FOUND {
            Ok(())
        } else {
            Err(rejected(response).await)
        }
    }
}

struct Measured {
    inner: ByteStream,
    remaining: u64,
    held: Option<Bytes>,
    done: bool,
    failed: Arc<AtomicBool>,
}

impl Measured {
    fn new(inner: ByteStream, length: u64, failed: Arc<AtomicBool>) -> Self {
        Self {
            inner,
            remaining: length,
            held: None,
            done: false,
            failed,
        }
    }

    fn fail(&mut self, err: ObjectStoreError) -> Poll<Option<Result<Bytes, ObjectStoreError>>> {
        self.done = true;
        self.failed.store(true, Ordering::Release);
        Poll::Ready(Some(Err(err)))
    }
}

impl Stream for Measured {
    type Item = Result<Bytes, ObjectStoreError>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            if self.done {
                return Poll::Ready(None);
            }
            let next = ready!(self.inner.as_mut().poll_next(cx));
            match (next, self.held.take()) {
                (Some(Ok(chunk)), held) if chunk.is_empty() => self.held = held,
                (Some(Ok(_)), Some(_)) => {
                    return self.fail(ObjectStoreError::body(WrongLength("longer")));
                }
                (Some(Ok(chunk)), None) => {
                    let len = u64::try_from(chunk.len()).unwrap_or(u64::MAX);
                    match self.remaining.checked_sub(len) {
                        None => return self.fail(ObjectStoreError::body(WrongLength("longer"))),
                        Some(0) => {
                            self.remaining = 0;
                            self.held = Some(chunk);
                        }
                        Some(remaining) => {
                            self.remaining = remaining;
                            return Poll::Ready(Some(Ok(chunk)));
                        }
                    }
                }
                (Some(Err(err)), _) => return self.fail(err),
                (None, held) => {
                    self.done = true;
                    return match held {
                        Some(last) => Poll::Ready(Some(Ok(last))),
                        None if self.remaining > 0 => {
                            self.fail(ObjectStoreError::body(WrongLength("shorter")))
                        }
                        None => Poll::Ready(None),
                    };
                }
            }
        }
    }
}
