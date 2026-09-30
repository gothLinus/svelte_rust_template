//! Request and response bodies of the files endpoints, and their validation. Like the notes'
//! DTOs, except that an upload's contents and a download's are not a message: they travel as the
//! raw request and response body ([`ByteStream`]), and the DTOs carry what the HTTP layer read
//! from the headers and the query string.

use std::fmt::{self, Debug, Formatter};

use domain::{
    file::{ContentType, FileChanges, FileFilter, FileName, FileSize, NewFile, StoredFile},
    object_store::{ByteStream, ObjectKey},
    pagination::{NewestFirst, PageRequest},
};
use serde::Deserialize;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    actor::Actor,
    crud::{Changes, Input},
    error::ValidationErrors,
    pagination::page_request,
};

/// A file as the API returns it. Maps to the `StoredFile` message; the contents are downloaded
/// separately.
#[derive(Debug, Clone)]
pub struct FileDto {
    pub id: Uuid,
    pub owner_id: Uuid,
    pub name: String,
    pub content_type: String,
    pub size: u64,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
}

impl From<StoredFile> for FileDto {
    fn from(file: StoredFile) -> Self {
        Self {
            id: file.id().as_uuid(),
            owner_id: file.owner_id().as_uuid(),
            name: file.name().to_string(),
            content_type: file.content_type().as_str().to_owned(),
            size: file.size().bytes(),
            created_at: file.created_at(),
            updated_at: file.updated_at(),
        }
    }
}

/// What an upload says about its contents: the name from the query string, the type and length
/// from the request's headers. The contents themselves are the body, passed alongside.
#[derive(Debug, Clone)]
pub struct UploadFileRequest {
    pub name: String,
    pub content_type: String,
    /// `Content-Length`: uploads need one, so the size is checked before a byte is read.
    pub size: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct ValidUpload {
    pub name: FileName,
    pub content_type: ContentType,
    pub size: FileSize,
}

impl UploadFileRequest {
    /// Every field checked at once. The size is reported on `file`, the field of the form it came
    /// from.
    pub(crate) fn validate(self) -> Result<ValidUpload, ValidationErrors> {
        let mut errors = ValidationErrors::new();
        let name = errors.check("name", FileName::parse(&self.name));
        let content_type = errors.check("contentType", ContentType::parse(&self.content_type));
        let size = errors.check("file", FileSize::new(self.size));

        match (name, content_type, size) {
            (Some(name), Some(content_type), Some(size)) => Ok(ValidUpload {
                name,
                content_type,
                size,
            }),
            _ => Err(errors),
        }
    }
}

pub(crate) struct StoredUpload {
    pub upload: ValidUpload,
    pub object_key: ObjectKey,
}

impl Input<NewFile> for StoredUpload {
    fn into_domain(self, actor: &Actor) -> Result<NewFile, ValidationErrors> {
        Ok(NewFile {
            owner_id: actor.user_id,
            name: self.upload.name,
            content_type: self.upload.content_type,
            size: self.upload.size,
            object_key: self.object_key,
        })
    }
}

/// Body of the update endpoint, a partial update: omitted fields keep their value. Converted to
/// `FileChanges`.
#[derive(Debug)]
pub struct UpdateFileRequest {
    pub name: Option<String>,
}

impl Changes<StoredFile> for UpdateFileRequest {
    fn into_changes(
        self,
        _actor: &Actor,
        _current: &StoredFile,
    ) -> Result<FileChanges, ValidationErrors> {
        let mut errors = ValidationErrors::new();
        let name = self
            .name
            .map(|name| errors.check("name", FileName::parse(&name)));

        if !errors.is_empty() {
            return Err(errors);
        }
        Ok(FileChanges {
            name: name.flatten(),
        })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileScope {
    #[default]
    Mine,
    All,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListFilesQuery {
    pub scope: Option<FileScope>,
    pub limit: Option<u32>,
    pub after: Option<String>,
}

impl ListFilesQuery {
    /// The filter as requested and the validated page; the policy then checks the filter (asking
    /// for `All` without `files:manage` is `Forbidden`).
    pub fn into_parts(self, actor: &Actor) -> Result<(FileFilter, PageRequest), ValidationErrors> {
        let page = page_request(self.limit, self.after.as_deref(), NewestFirst)?;
        let filter = match self.scope.unwrap_or_default() {
            FileScope::Mine => FileFilter {
                owner_id: Some(actor.user_id),
            },
            FileScope::All => FileFilter { owner_id: None },
        };
        Ok((filter, page))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileUsageDto {
    pub used_bytes: u64,
    pub quota_bytes: Option<u64>,
}

/// A file's contents on their way to the client, with what the response says about them. The HTTP
/// layer streams `body` as the response.
pub struct FileDownload {
    pub name: String,
    pub content_type: String,
    pub size: u64,
    pub body: ByteStream,
}

impl Debug for FileDownload {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileDownload")
            .field("name", &self.name)
            .field("content_type", &self.content_type)
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}
