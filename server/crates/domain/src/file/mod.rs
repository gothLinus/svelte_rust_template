//! Files: what a user uploaded, owned by them. The row ([`StoredFile`]) holds what the app
//! shows; the contents live in the [`ObjectStore`](crate::object_store::ObjectStore) under the
//! file's [`ObjectKey`].
//!
//! A resource like [`crate::note`], plus contents: its [`Resource`] impl gives it the generic
//! list, read, rename and delete, while uploads and downloads are use cases of their own in
//! `application::files`. The rest of the slice:
//!
//! - `application::files`: the service, the DTOs and the policy (owner or `files:manage`);
//! - `infrastructure::db::repositories::files`: the Postgres `Repository<StoredFile>` and
//!   [`FileRepository`];
//! - `api`: `routes/files.rs` (handlers; contents as raw bytes) and `wire/files.rs`, with the
//!   messages in `proto/api/v1/files.proto`;
//! - `server/migrations/*_files.up.sql`: the table, and the trigger that queues a deleted file's
//!   object for removal;
//! - `web/src/lib`: `api/files.ts` and the page under `routes/(app)/files/`.

use std::{
    fmt::{self, Display, Formatter},
    future::Future,
};

use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    error::{StorageError, ValidationError},
    i18n::Message,
    id::Id,
    object_store::ObjectKey,
    pagination::NewestFirst,
    repository::Resource,
    unicode::is_bidi_override,
    user::UserId,
};

pub type FileId = Id<StoredFile>;

pub const MAX_FILE_NAME_LEN: usize = 255;
pub const MAX_FILE_SIZE: u64 = 25 * 1024 * 1024;
pub const MAX_CONTENT_TYPE_LEN: usize = 255;
pub const OBJECT_KEY_PREFIX: &str = "files";

/// What a file is called, as shown and downloaded: trimmed, 1 to [`MAX_FILE_NAME_LEN`]
/// characters, without control characters, bidirectional overrides or path separators, and not
/// `.` or `..`. Whatever the browser named the upload, a download never names a path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileName(String);

impl FileName {
    pub fn parse(raw: &str) -> Result<Self, ValidationError> {
        let name = raw.trim();
        let len = name.chars().count();

        if len == 0 {
            Err(ValidationError::required())
        } else if len > MAX_FILE_NAME_LEN {
            Err(ValidationError::new(
                "too_long",
                Message::new("file-name-too-long").arg("max", MAX_FILE_NAME_LEN),
            ))
        } else if name == "."
            || name == ".."
            || name
                .chars()
                .any(|c| c.is_control() || is_bidi_override(c) || c == '/' || c == '\\')
        {
            Err(ValidationError::new(
                "invalid_characters",
                Message::new("file-name-invalid"),
            ))
        } else {
            Ok(Self(name.to_owned()))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Display for FileName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A media type without parameters, lowercased: `image/png`. What the browser said the upload
/// is; downloads repeat it but always as an attachment, so a file claiming to be a web page is
/// saved, never rendered on the app's origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentType(String);

impl ContentType {
    pub const OCTET_STREAM: &'static str = "application/octet-stream";

    /// Parses `type/subtype`, dropping parameters such as `; charset=utf-8`. Empty is
    /// [`ContentType::OCTET_STREAM`].
    pub fn parse(raw: &str) -> Result<Self, ValidationError> {
        let essence = raw.split(';').next().unwrap_or_default().trim();
        if essence.is_empty() {
            return Ok(Self(Self::OCTET_STREAM.to_owned()));
        }
        let valid = essence.len() <= MAX_CONTENT_TYPE_LEN
            && essence
                .split_once('/')
                .is_some_and(|(kind, subtype)| is_token(kind) && is_token(subtype));
        if valid {
            Ok(Self(essence.to_ascii_lowercase()))
        } else {
            Err(ValidationError::new(
                "invalid_content_type",
                Message::new("file-content-type-invalid"),
            ))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

fn is_token(part: &str) -> bool {
    !part.is_empty()
        && part
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
}

/// A file's length in bytes, at most [`MAX_FILE_SIZE`]. Empty files are allowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FileSize(u64);

impl FileSize {
    pub fn new(bytes: u64) -> Result<Self, ValidationError> {
        if bytes > MAX_FILE_SIZE {
            Err(too_large())
        } else {
            Ok(Self(bytes))
        }
    }

    pub const fn bytes(self) -> u64 {
        self.0
    }
}

pub fn too_large() -> ValidationError {
    ValidationError::new(
        "too_large",
        Message::new("file-too-large").arg(
            "max",
            i64::try_from(MAX_FILE_SIZE / (1024 * 1024)).unwrap_or(i64::MAX),
        ),
    )
}

/// A file as stored. Its owner and holders of `files:manage` may access it; the application
/// layer's policy enforces that, not this type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredFile {
    id: FileId,
    owner_id: UserId,
    name: FileName,
    content_type: ContentType,
    size: FileSize,
    object_key: ObjectKey,
    created_at: OffsetDateTime,
    updated_at: OffsetDateTime,
}

pub struct StoredFileParts {
    pub id: FileId,
    pub owner_id: UserId,
    pub name: FileName,
    pub content_type: ContentType,
    pub size: FileSize,
    pub object_key: ObjectKey,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

impl StoredFile {
    pub fn from_parts(parts: StoredFileParts) -> Self {
        Self {
            id: parts.id,
            owner_id: parts.owner_id,
            name: parts.name,
            content_type: parts.content_type,
            size: parts.size,
            object_key: parts.object_key,
            created_at: parts.created_at,
            updated_at: parts.updated_at,
        }
    }

    pub fn id(&self) -> FileId {
        self.id
    }

    pub fn owner_id(&self) -> UserId {
        self.owner_id
    }

    pub fn name(&self) -> &FileName {
        &self.name
    }

    pub fn content_type(&self) -> &ContentType {
        &self.content_type
    }

    pub fn size(&self) -> FileSize {
        self.size
    }

    pub fn object_key(&self) -> &ObjectKey {
        &self.object_key
    }

    pub fn created_at(&self) -> OffsetDateTime {
        self.created_at
    }

    pub fn updated_at(&self) -> OffsetDateTime {
        self.updated_at
    }
}

/// A new key for an upload's contents. Keys are not made from the file's id, because the
/// contents are stored before the row that gets the id.
pub fn new_object_key() -> ObjectKey {
    ObjectKey::new(OBJECT_KEY_PREFIX, Uuid::now_v7())
}

/// Validated input for the file's row ([`Resource::Create`]), once its contents are in the store
/// under `object_key`.
#[derive(Debug, Clone)]
pub struct NewFile {
    pub owner_id: UserId,
    pub name: FileName,
    pub content_type: ContentType,
    pub size: FileSize,
    pub object_key: ObjectKey,
}

/// A partial update: `None` leaves the field as it is. Only the name changes; new contents are a
/// new upload.
#[derive(Debug, Clone, Default)]
pub struct FileChanges {
    pub name: Option<FileName>,
}

impl FileChanges {
    pub fn is_empty(&self) -> bool {
        self.name.is_none()
    }
}

/// Narrows a file list. `Default` lists every file, so services must always scope it through the
/// policy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FileFilter {
    pub owner_id: Option<UserId>,
}

impl Resource for StoredFile {
    type Id = FileId;
    type Create = NewFile;
    type Update = FileChanges;
    type Filter = FileFilter;
    type Sort = NewestFirst;
}

pub trait FileRepository: Send {
    fn stored_bytes(
        &mut self,
        owner: UserId,
    ) -> impl Future<Output = Result<u64, StorageError>> + Send;

    fn lock_storage(
        &mut self,
        owner: UserId,
    ) -> impl Future<Output = Result<(), StorageError>> + Send;
}
