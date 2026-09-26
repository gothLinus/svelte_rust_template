use application::{
    actor::Actor,
    notes::NotePolicy,
    policy::{Action, SimplePolicy},
};
use domain::{
    note::{Note, NoteBody, NoteFilter, NoteId, NoteParts, NoteTitle},
    rbac::{Permission, PermissionSet},
    session::SessionId,
    user::UserId,
};
use time::OffsetDateTime;

fn actor(permissions: &[Permission]) -> Actor {
    Actor {
        user_id: UserId::generate(),
        session_id: SessionId::generate(),
        permissions: permissions.iter().copied().collect(),
        email_verified: true,
        recently_authenticated: true,
    }
}

fn note_of(owner: UserId) -> Note {
    Note::from_parts(NoteParts {
        id: NoteId::generate(),
        owner_id: owner,
        title: NoteTitle::parse("t").unwrap(),
        body: NoteBody::default(),
        created_at: OffsetDateTime::UNIX_EPOCH,
        updated_at: OffsetDateTime::UNIX_EPOCH,
    })
}

#[test]
fn owners_with_read_and_write() {
    let user = actor(&[Permission::NotesRead, Permission::NotesWrite]);
    let own = note_of(user.user_id);
    let foreign = note_of(UserId::generate());

    assert!(NotePolicy::can(&user, Action::Read, None));
    assert!(NotePolicy::can(&user, Action::Create, None));
    for action in [Action::Read, Action::Update, Action::Delete] {
        assert!(NotePolicy::can(&user, action, Some(&own)), "{action:?}");
        assert!(
            !NotePolicy::can(&user, action, Some(&foreign)),
            "{action:?}"
        );
    }
}

#[test]
fn managers_act_on_everything() {
    let manager = actor(&[Permission::NotesManage]);
    let foreign = note_of(UserId::generate());

    for action in [Action::Read, Action::Update, Action::Delete] {
        assert!(
            NotePolicy::can(&manager, action, Some(&foreign)),
            "{action:?}"
        );
    }
    assert!(NotePolicy::can(&manager, Action::Read, None));
    // Managing others' notes does not imply writing your own.
    assert!(!NotePolicy::can(&manager, Action::Create, None));
}

#[test]
fn read_only_users_cannot_change_even_their_own() {
    let reader = actor(&[Permission::NotesRead]);
    let own = note_of(reader.user_id);

    assert!(NotePolicy::can(&reader, Action::Read, Some(&own)));
    assert!(!NotePolicy::can(&reader, Action::Update, Some(&own)));
    assert!(!NotePolicy::can(&reader, Action::Delete, Some(&own)));
    assert!(!NotePolicy::can(&reader, Action::Create, None));
}

#[test]
fn nobody_without_permissions_gets_anything() {
    let nobody = Actor {
        permissions: PermissionSet::empty(),
        ..actor(&[])
    };
    let own = note_of(nobody.user_id);
    for action in [Action::Read, Action::Create, Action::Update, Action::Delete] {
        assert!(!NotePolicy::can(&nobody, action, None));
        assert!(!NotePolicy::can(&nobody, action, Some(&own)));
    }
}

#[test]
fn updates_and_deletes_need_a_resource() {
    let manager = actor(&[Permission::NotesManage, Permission::NotesWrite]);
    assert!(!NotePolicy::can(&manager, Action::Update, None));
    assert!(!NotePolicy::can(&manager, Action::Delete, None));
}

#[test]
fn list_scope() {
    let user = actor(&[Permission::NotesRead]);
    let manager = actor(&[Permission::NotesManage]);
    let own = NoteFilter {
        owner_id: Some(user.user_id),
    };
    let everyone = NoteFilter { owner_id: None };
    let someone_else = NoteFilter {
        owner_id: Some(UserId::generate()),
    };

    assert_eq!(NotePolicy::scope(&user, own), Some(own));
    assert_eq!(NotePolicy::scope(&user, everyone), None);
    assert_eq!(NotePolicy::scope(&user, someone_else), None);
    assert_eq!(NotePolicy::scope(&manager, everyone), Some(everyone));
    assert_eq!(
        NotePolicy::scope(&manager, someone_else),
        Some(someone_else)
    );
}

#[test]
fn actor_require() {
    let user = actor(&[Permission::NotesRead]);
    assert!(user.require(Permission::NotesRead).is_ok());
    assert_eq!(
        user.require(Permission::UsersManage).unwrap_err().code(),
        "forbidden"
    );
}
