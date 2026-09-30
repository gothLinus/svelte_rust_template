use std::sync::Arc;

use domain::{
    clock::Clock,
    database::{Database, Transaction},
    error::ErrorChain,
    file::{FileId, FileRepository, FileSize, StoredFile, new_object_key},
    i18n::Message,
    object_store::{
        ByteStream, NewObject, ObjectDeletionRepository, ObjectKey, ObjectStore, ObjectStoreError,
    },
    repository::Repository,
};
use thiserror::Error;
use time::Duration;

use crate::{
    Adapters, Context,
    actor::Actor,
    crud::CrudService,
    error::{AppError, InternalError},
    files::{
        FilePolicy,
        dto::{
            FileDownload, FileDto, FileUsageDto, ListFilesQuery, StoredUpload, UpdateFileRequest,
            UploadFileRequest,
        },
    },
    pagination::PageDto,
    policy::{Action, SimplePolicy},
};

/// How long an upload's key stays queued for deletion before the maintenance job removes what it
/// stored: far longer than an upload may take (`UPLOAD_TIMEOUT` is at most an hour), so a slow
/// upload is never deleted under it. A finished upload takes its key off.
pub const UNFINISHED_UPLOAD_TTL: Duration = Duration::hours(6);

#[derive(Debug, Error)]
#[error("the object `{0}` of a stored file is missing")]
struct MissingObject(ObjectKey);

/// Files: CRUD for the rows through [`CrudService`], and the contents through the [`ObjectStore`].
///
/// Each method takes the [`Actor`] of the request and returns a response DTO; the failures are
/// those of [`CrudService`] (`NotFound`, `Forbidden`, `Validation`), plus the upload's own (see
/// [`FileService::upload`]).
pub struct FileService<A: Adapters> {
    crud: CrudService<A, StoredFile, FilePolicy>,
}

impl<A: Adapters> FileService<A> {
    pub fn new(ctx: Arc<Context<A>>) -> Self {
        Self {
            crud: CrudService::new(ctx),
        }
    }

    fn ctx(&self) -> &Context<A> {
        self.crud.context()
    }

    pub async fn list(
        &self,
        actor: &Actor,
        query: ListFilesQuery,
    ) -> Result<PageDto<FileDto>, AppError> {
        let (filter, page) = query.into_parts(actor)?;
        let page = self.crud.list(actor, filter, page).await?;
        Ok(page.into())
    }

    pub async fn get(&self, actor: &Actor, id: FileId) -> Result<FileDto, AppError> {
        self.crud.get(actor, id).await
    }

    pub async fn update(
        &self,
        actor: &Actor,
        id: FileId,
        request: UpdateFileRequest,
    ) -> Result<FileDto, AppError> {
        self.crud.update(actor, id, request).await
    }

    /// Stores a new file owned by the actor: its contents from `body`, which yields the
    /// `request.size` bytes the request announced, then its row.
    ///
    /// Everything that can be refused is refused before a byte is read: the permission, the name,
    /// type and size, and the quota. The key is queued for deletion before the contents are stored
    /// and taken off in the transaction that stores the row, so an upload that fails halfway, or a
    /// server that dies during one, leaves nothing behind for long (see [`UNFINISHED_UPLOAD_TTL`]).
    /// No connection is held while the contents stream in.
    ///
    /// The quota is checked again in that transaction, under [`FileRepository::lock_storage`]:
    /// uploads running side by side each fit when they start, so only the second check sees what
    /// the others stored meanwhile.
    ///
    /// # Errors
    ///
    /// `Forbidden` without `files:write`; `Validation` on `name`, `contentType` or `file` (too
    /// large); `Conflict` (`file_quota_exceeded`) when the file does not fit the user's quota;
    /// `IncompleteUpload` if the body ends early.
    pub async fn upload(
        &self,
        actor: &Actor,
        request: UploadFileRequest,
        body: ByteStream,
    ) -> Result<FileDto, AppError> {
        if !FilePolicy::can(actor, Action::Create, None) {
            return Err(AppError::Forbidden);
        }
        let upload = request.validate()?;
        let ctx = self.ctx();

        let object_key = new_object_key();
        let mut conn = ctx.db.connection().await?;
        self.check_quota(&mut conn, actor, upload.size).await?;
        let give_up_at = ctx.clock.now() + UNFINISHED_UPLOAD_TTL;
        conn.schedule_object_deletion(&object_key, Some(give_up_at))
            .await?;
        drop(conn);

        ctx.objects
            .put(
                &object_key,
                NewObject {
                    content_type: upload.content_type.as_str().to_owned(),
                    length: upload.size.bytes(),
                    body,
                },
            )
            .await?;

        let mut tx = ctx.db.transaction().await?;
        tx.lock_storage(actor.user_id).await?;
        if let Err(err) = self.check_quota(&mut tx, actor, upload.size).await {
            drop(tx);
            discard_object(ctx, &object_key).await;
            return Err(err);
        }
        let file = self
            .crud
            .create_in(
                &mut tx,
                actor,
                StoredUpload {
                    upload,
                    object_key: object_key.clone(),
                },
            )
            .await?;
        tx.cancel_object_deletion(&object_key).await?;
        tx.commit().await?;
        Ok(file)
    }

    async fn check_quota<S: FileRepository>(
        &self,
        store: &mut S,
        actor: &Actor,
        size: FileSize,
    ) -> Result<(), AppError> {
        let Some(quota) = self.ctx().settings.file_quota else {
            return Ok(());
        };
        let used = store.stored_bytes(actor.user_id).await?;
        if used.saturating_add(size.bytes()) > quota {
            return Err(AppError::conflict(
                "file_quota_exceeded",
                Message::new("conflict-file-quota-exceeded"),
            ));
        }
        Ok(())
    }

    /// The file's contents, for a download: authorized like reading the file.
    ///
    /// # Errors
    ///
    /// `NotFound` for a file the actor may not see; `Internal` if the store fails or does not have
    /// the contents.
    pub async fn download(&self, actor: &Actor, id: FileId) -> Result<FileDownload, AppError> {
        let file = self.crud.authorize(actor, id, Action::Read).await?;
        let object = self
            .ctx()
            .objects
            .get(file.object_key())
            .await?
            .ok_or_else(|| {
                let missing = MissingObject(file.object_key().clone());
                InternalError::Objects(ObjectStoreError::backend(missing))
            })?;

        Ok(FileDownload {
            name: file.name().to_string(),
            content_type: file.content_type().as_str().to_owned(),
            // The row's size, not the store's: it is what the upload was checked against, and a
            // store answering without a length must not make the response lie.
            size: file.size().bytes(),
            body: object.body,
        })
    }

    /// Deletes the file's row, then its contents. The row's deletion queues the contents for
    /// removal in the same transaction, so if removing them now fails, the maintenance job does it
    /// later: a deleted file never lingers in the store.
    pub async fn delete(&self, actor: &Actor, id: FileId) -> Result<(), AppError> {
        let ctx = self.ctx();
        let mut tx = ctx.db.transaction().await?;
        let file = self
            .crud
            .authorize_in(&mut tx, actor, id, Action::Delete)
            .await?;
        if !Repository::<StoredFile>::delete(&mut tx, id).await? {
            return Err(AppError::NotFound);
        }
        tx.commit().await?;

        discard_object(ctx, file.object_key()).await;
        Ok(())
    }

    pub async fn usage(&self, actor: &Actor) -> Result<FileUsageDto, AppError> {
        if !FilePolicy::can(actor, Action::Read, None) {
            return Err(AppError::Forbidden);
        }
        let used_bytes = self
            .ctx()
            .db
            .connection()
            .await?
            .stored_bytes(actor.user_id)
            .await?;
        Ok(FileUsageDto {
            used_bytes,
            quota_bytes: self.ctx().settings.file_quota,
        })
    }
}

async fn discard_object<A: Adapters>(ctx: &Context<A>, key: &ObjectKey) {
    if let Err(err) = purge_object(ctx, key).await {
        tracing::warn!(
            error = %ErrorChain(&err),
            %key,
            "removing a file's contents failed; maintenance retries it"
        );
    }
}

/// Removes the object under `key` from the store, then from the deletion queue. A crash between
/// the two leaves the key queued, and removing it again does no harm.
pub(crate) async fn purge_object<A: Adapters>(
    ctx: &Context<A>,
    key: &ObjectKey,
) -> Result<(), AppError> {
    ctx.objects.delete(key).await?;
    ctx.db
        .connection()
        .await?
        .cancel_object_deletion(key)
        .await?;
    Ok(())
}
