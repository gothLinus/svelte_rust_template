//! Conversions between the files' DTOs and their protobuf messages, like `notes.rs`. Only the
//! descriptions of files are messages: uploads and downloads carry the contents as raw bytes (see
//! `routes/files.rs`), which no encoding could make cheaper.

use application::{
    ValidationErrors,
    files::dto::{FileDto, FileUsageDto, UpdateFileRequest},
};
use proto::v1;

use super::{FromMessage, IntoMessage, page_message, timestamp};

impl IntoMessage for FileDto {
    type Message = v1::StoredFile;

    fn into_message(self) -> v1::StoredFile {
        v1::StoredFile {
            id: self.id.to_string(),
            owner_id: self.owner_id.to_string(),
            name: self.name,
            content_type: self.content_type,
            size: self.size,
            created_at: Some(timestamp(self.created_at)),
            updated_at: Some(timestamp(self.updated_at)),
        }
    }
}

page_message!(FileDto => v1::StoredFilePage);

impl IntoMessage for FileUsageDto {
    type Message = v1::FileUsage;

    fn into_message(self) -> v1::FileUsage {
        v1::FileUsage {
            used_bytes: self.used_bytes,
            quota_bytes: self.quota_bytes,
        }
    }
}

impl FromMessage for UpdateFileRequest {
    type Message = v1::UpdateFileRequest;

    fn from_message(message: v1::UpdateFileRequest) -> Result<Self, ValidationErrors> {
        Ok(Self { name: message.name })
    }
}
