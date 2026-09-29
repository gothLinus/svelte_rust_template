use domain::{
    note::{Note, NoteFilter},
    rbac::Permission,
    user::UserId,
};

use crate::{
    actor::Actor,
    policy::{Action, Owned, SimplePolicy, owner_or},
};

impl Owned for Note {
    fn owner_id(&self) -> UserId {
        Note::owner_id(self)
    }
}

/// - `notes:read` / `notes:write`: read / change your own notes.
/// - `notes:manage`: read and change everyone's.
/// - Anyone with `notes:write` may create; a list is limited to the actor's own notes unless they
///   hold `notes:manage` (asking for others' is `Forbidden`).
///
/// Copy and adapt the match arms when the rules differ; a policy that needs more than the actor's
/// permissions and the resource implements [`Policy`](crate::policy::Policy) instead.
pub struct NotePolicy;

impl SimplePolicy<Note> for NotePolicy {
    fn can(actor: &Actor, action: Action, note: Option<&Note>) -> bool {
        use Permission::{NotesManage, NotesRead, NotesWrite};

        match (action, note) {
            (Action::Read, None) => actor.has(NotesRead) || actor.has(NotesManage),
            (Action::Read, Some(note)) => owner_or(actor, note, NotesRead, NotesManage),
            (Action::Create, _) => actor.has(NotesWrite),
            (Action::Update | Action::Delete, Some(note)) => {
                owner_or(actor, note, NotesWrite, NotesManage)
            }
            (Action::Update | Action::Delete, None) => false,
        }
    }

    fn scope(actor: &Actor, filter: NoteFilter) -> Option<NoteFilter> {
        if actor.has(Permission::NotesManage) {
            return Some(filter);
        }
        match filter.owner_id {
            Some(owner) if owner == actor.user_id => Some(filter),
            _ => None,
        }
    }
}
