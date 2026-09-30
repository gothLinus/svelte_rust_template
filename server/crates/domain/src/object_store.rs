//! The object storage port: file contents, kept in an S3-compatible bucket rather than in the
//! database.
//!
//! - [`ObjectStore`] reads and writes the bytes. Contents stream through in chunks
//!   ([`ByteStream`]), so a large upload or download never sits in memory whole.
//! - [`ObjectDeletionRepository`] is the database's queue of objects to remove. The store takes
//!   no part in a database transaction, so a use case never deletes an object directly: it
//!   removes the row that names it (a trigger queues the key in the same transaction) and the
//!   maintenance job empties the queue. An upload queues its own key before it starts and takes
//!   it off once its row is committed, so a failed upload is cleaned up the same way.

use std::{
    error::Error as StdError,
    fmt::{self, Display, Formatter},
    future::Future,
    pin::Pin,
};

use bytes::Bytes;
use futures_core::Stream;
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::{StorageError, UnknownValue};

pub const MAX_OBJECT_KEY_LEN: usize = 256;

/// Where an object lives in the bucket, such as `files/0192b9c4-…`: 1 to [`MAX_OBJECT_KEY_LEN`]
/// ASCII letters, digits and `/ - _ .`, not starting or ending with `/`. Keys are made by the
/// code, never taken from a user, so they need no escaping in a URL or a log line.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjectKey(String);

impl ObjectKey {
    /// A new key `<prefix>/<id>`, with one prefix per kind of content.
    ///
    /// # Panics
    ///
    /// In debug builds, if `prefix` breaks the rules of [`ObjectKey`]; prefixes are constants, so a
    /// test catches it.
    pub fn new(prefix: &'static str, id: Uuid) -> Self {
        let key = format!("{prefix}/{}", id.hyphenated());
        debug_assert!(is_valid_key(&key), "`{prefix}` is not a valid key prefix");
        Self(key)
    }

    /// A key read back from storage.
    ///
    /// # Errors
    ///
    /// Fails if `raw` breaks the rules of [`ObjectKey`], which only data written by something other
    /// than this code can do.
    pub fn parse(raw: &str) -> Result<Self, UnknownValue> {
        if is_valid_key(raw) {
            Ok(Self(raw.to_owned()))
        } else {
            Err(UnknownValue::new("object key", raw))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for ObjectKey {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn is_valid_key(raw: &str) -> bool {
    (1..=MAX_OBJECT_KEY_LEN).contains(&raw.len())
        && !raw.starts_with('/')
        && !raw.ends_with('/')
        && !raw.contains("//")
        && raw
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'/' | b'-' | b'_' | b'.'))
}

#[derive(Debug, Error)]
pub enum ObjectStoreError {
    /// The contents being written ended before the promised length or the stream failed, usually
    /// because a client went away mid-upload. Nothing was stored.
    #[error("the object's contents ended early")]
    Body(#[source] Box<dyn StdError + Send + Sync>),
    #[error("object storage failed")]
    Backend(#[source] Box<dyn StdError + Send + Sync>),
}

impl ObjectStoreError {
    pub fn body(err: impl StdError + Send + Sync + 'static) -> Self {
        Self::Body(Box::new(err))
    }

    pub fn backend(err: impl StdError + Send + Sync + 'static) -> Self {
        Self::Backend(Box::new(err))
    }
}

/// An object's contents, in chunks as they arrive. `Bytes` are reference-counted, so the bytes a
/// client sent reach the store without a copy.
pub type ByteStream = Pin<Box<dyn Stream<Item = Result<Bytes, ObjectStoreError>> + Send>>;

pub struct NewObject {
    pub content_type: String,
    /// Exactly how many bytes `body` yields. Stores need the length up front, and a body that
    /// yields fewer or more fails the write ([`ObjectStoreError::Body`]).
    pub length: u64,
    pub body: ByteStream,
}

impl fmt::Debug for NewObject {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("NewObject")
            .field("content_type", &self.content_type)
            .field("length", &self.length)
            .finish_non_exhaustive()
    }
}

/// A stored object as [`ObjectStore::get`] returns it: its length and its contents, still to be
/// read.
pub struct Object {
    pub length: u64,
    pub body: ByteStream,
}

impl fmt::Debug for Object {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Object")
            .field("length", &self.length)
            .finish_non_exhaustive()
    }
}

/// Stores file contents under [`ObjectKey`]s.
///
/// Statically dispatched through `application::Adapters`: production talks S3
/// (`infrastructure::object_store::S3ObjectStore`), tests use an in-memory store.
/// Implementations must stream: neither [`ObjectStore::put`] nor [`ObjectStore::get`] may buffer
/// a whole object.
pub trait ObjectStore: Send + Sync + 'static {
    /// Stores `object` under `key`, replacing anything there. `Ok` only once the store has every
    /// byte.
    ///
    /// # Errors
    ///
    /// [`ObjectStoreError::Body`] if the contents ended early or failed, `Backend` if the store did
    /// not take them.
    fn put(
        &self,
        key: &ObjectKey,
        object: NewObject,
    ) -> impl Future<Output = Result<(), ObjectStoreError>> + Send;

    fn get(
        &self,
        key: &ObjectKey,
    ) -> impl Future<Output = Result<Option<Object>, ObjectStoreError>> + Send;

    fn delete(&self, key: &ObjectKey) -> impl Future<Output = Result<(), ObjectStoreError>> + Send;
}

/// The queue of objects to remove from the [`ObjectStore`], kept in the database (see the module
/// docs). Implemented on the same connection type as the repositories, so a use case queues or
/// cancels a key in the transaction that writes the row naming it.
pub trait ObjectDeletionRepository: Send {
    /// Queues `key` for deletion once `due_at` has passed, or as soon as possible for `None`.
    /// Queueing a key again moves its time to the new one.
    fn schedule_object_deletion(
        &mut self,
        key: &ObjectKey,
        due_at: Option<OffsetDateTime>,
    ) -> impl Future<Output = Result<(), StorageError>> + Send;

    /// Takes `key` off the queue because its upload finished or the object is gone. Returns whether
    /// it was queued.
    fn cancel_object_deletion(
        &mut self,
        key: &ObjectKey,
    ) -> impl Future<Output = Result<bool, StorageError>> + Send;

    fn due_object_deletions(
        &mut self,
        now: OffsetDateTime,
        limit: u32,
    ) -> impl Future<Output = Result<Vec<ObjectKey>, StorageError>> + Send;
}
