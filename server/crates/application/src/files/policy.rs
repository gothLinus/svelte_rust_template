use domain::{
    file::{FileFilter, StoredFile},
    rbac::Permission,
    user::UserId,
};

use crate::{
    actor::Actor,
    policy::{Action, Owned, SimplePolicy, owner_or},
};

impl Owned for StoredFile {
    fn owner_id(&self) -> UserId {
        StoredFile::owner_id(self)
    }
}

/// - `files:read` / `files:write`: download / upload, rename and delete your own files.
/// - `files:manage`: read and change everyone's.
/// - Anyone with `files:write` may upload; a list is limited to the actor's own files unless they
///   hold `files:manage` (asking for others' is `Forbidden`).
pub struct FilePolicy;

impl SimplePolicy<StoredFile> for FilePolicy {
    fn can(actor: &Actor, action: Action, file: Option<&StoredFile>) -> bool {
        use Permission::{FilesManage, FilesRead, FilesWrite};

        match (action, file) {
            (Action::Read, None) => actor.has(FilesRead) || actor.has(FilesManage),
            (Action::Read, Some(file)) => owner_or(actor, file, FilesRead, FilesManage),
            (Action::Create, _) => actor.has(FilesWrite),
            (Action::Update | Action::Delete, Some(file)) => {
                owner_or(actor, file, FilesWrite, FilesManage)
            }
            (Action::Update | Action::Delete, None) => false,
        }
    }

    fn scope(actor: &Actor, filter: FileFilter) -> Option<FileFilter> {
        if actor.has(Permission::FilesManage) {
            return Some(filter);
        }
        match filter.owner_id {
            Some(owner) if owner == actor.user_id => Some(filter),
            _ => None,
        }
    }
}
