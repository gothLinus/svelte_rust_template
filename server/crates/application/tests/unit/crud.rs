use std::sync::Arc;

use application::{
    AppError, Store,
    actor::Actor,
    crud::{Changes, CrudService, Present},
    notes::{
        NotePolicy,
        dto::{CreateNoteRequest, NoteDto},
    },
    policy::{Action, Policy},
};
use domain::{
    database::{Database, Transaction},
    error::StorageError,
    i18n::Message,
    note::{Note, NoteChanges, NoteFilter, NoteId, NoteTitle},
    pagination::PageRequest,
    rbac::RoleName,
};

use crate::support::{Fakes, Fixture};

/// A relationship rule: only admins may change notes, whatever their permissions say. The admin
/// role is looked up through the store, once per operation.
struct AdminsOnly;

impl Policy<Note> for AdminsOnly {
    type Facts = bool;

    async fn facts(
        store: &mut impl Store,
        actor: &Actor,
        _note: Option<&Note>,
    ) -> Result<bool, StorageError> {
        Ok(store
            .roles_of_users(&[actor.user_id])
            .await?
            .iter()
            .any(|(_, role)| *role == RoleName::ADMIN))
    }

    fn allows(actor: &Actor, is_admin: &bool, action: Action, note: Option<&Note>) -> bool {
        match action {
            Action::Read => note.is_none_or(|note| *is_admin || note.owner_id() == actor.user_id),
            Action::Create | Action::Update | Action::Delete => *is_admin,
        }
    }

    fn narrow(_actor: &Actor, _is_admin: &bool, filter: NoteFilter) -> Option<NoteFilter> {
        Some(filter)
    }
}

struct NoteWithAccess {
    note: NoteDto,
    can_edit: bool,
}

impl Present<Note> for NoteWithAccess {
    fn present(note: Note, actor: &Actor) -> Self {
        Self {
            can_edit: note.owner_id() == actor.user_id,
            note: NoteDto::from(note),
        }
    }
}

struct Retitle(&'static str);

impl Changes<Note> for Retitle {
    fn into_changes(
        self,
        _actor: &Actor,
        current: &Note,
    ) -> Result<NoteChanges, application::ValidationErrors> {
        let title = NoteTitle::parse(self.0)
            .map_err(|err| application::ValidationErrors::single("title", &err))?;
        if title.as_str().len() < current.title().as_str().len() {
            return Err(application::ValidationErrors::single(
                "title",
                &domain::error::ValidationError::new(
                    "shorter",
                    Message::new("test-titles-only-grow"),
                ),
            ));
        }
        Ok(NoteChanges {
            title: Some(title),
            body: None,
        })
    }
}

fn service<P: Policy<Note>>(fx: &Fixture) -> CrudService<Fakes, Note, P> {
    CrudService::new(Arc::clone(fx.services.context()))
}

fn create(title: &str) -> CreateNoteRequest {
    CreateNoteRequest {
        title: title.to_owned(),
        body: None,
    }
}

#[tokio::test]
async fn policies_can_decide_from_facts_they_load() {
    let fx = Fixture::new();
    let admin = fx.admin("admin@example.com").await;
    let alice = fx.user("alice@example.com").await;
    let crud = service::<AdminsOnly>(&fx);

    let err = crud
        .create::<_, NoteDto>(&alice.actor, create("mine"))
        .await
        .err()
        .unwrap();
    assert!(matches!(err, AppError::Forbidden));
    let created: NoteDto = crud.create(&admin.actor, create("theirs")).await.unwrap();
    let id = NoteId::from_uuid(created.id);

    let err = crud.get::<NoteDto>(&alice.actor, id).await.err().unwrap();
    assert!(matches!(err, AppError::NotFound));
    crud.delete(&admin.actor, id).await.unwrap();
}

#[tokio::test]
async fn output_and_changes_can_depend_on_the_actor_and_the_entity() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    let manager = fx.admin("manager@example.com").await;
    let crud = service::<NotePolicy>(&fx);

    let created: NoteWithAccess = crud.create(&alice.actor, create("Plan")).await.unwrap();
    assert!(created.can_edit);
    let id = NoteId::from_uuid(created.note.id);
    let seen: NoteWithAccess = crud.get(&manager.actor, id).await.unwrap();
    assert!(!seen.can_edit);

    let err = crud
        .update::<_, NoteDto>(&alice.actor, id, Retitle("P"))
        .await
        .err()
        .unwrap();
    assert!(matches!(err, AppError::Validation(_)));
    let grown: NoteDto = crud
        .update(&alice.actor, id, Retitle("Plan B"))
        .await
        .unwrap();
    assert_eq!(grown.title, "Plan B");
}

#[tokio::test]
async fn authorize_is_the_policy_check_for_custom_use_cases() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    let bob = fx.user("bob@example.com").await;
    let manager = fx.admin("manager@example.com").await;
    let crud = service::<NotePolicy>(&fx);
    let note: NoteDto = crud.create(&alice.actor, create("private")).await.unwrap();
    let id = NoteId::from_uuid(note.id);

    assert_eq!(
        crud.authorize(&alice.actor, id, Action::Update)
            .await
            .unwrap()
            .id(),
        id
    );
    for action in [Action::Read, Action::Update, Action::Delete] {
        let err = crud.authorize(&bob.actor, id, action).await.unwrap_err();
        assert!(matches!(err, AppError::NotFound), "{action:?}");
    }
    let read_only = Actor {
        permissions: [domain::rbac::Permission::NotesRead].into_iter().collect(),
        ..alice.actor.clone()
    };
    let err = crud
        .authorize(&read_only, id, Action::Update)
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::Forbidden));
    assert!(
        crud.authorize(&manager.actor, id, Action::Delete)
            .await
            .is_ok()
    );
    assert!(matches!(
        crud.authorize(&alice.actor, NoteId::generate(), Action::Read)
            .await
            .unwrap_err(),
        AppError::NotFound
    ));
}

#[tokio::test]
async fn operations_can_share_the_callers_transaction() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    let crud = service::<NotePolicy>(&fx);

    let mut tx = fx.db.transaction().await.unwrap();
    let _first: NoteDto = crud
        .create_in(&mut tx, &alice.actor, create("one"))
        .await
        .unwrap();
    let _second: NoteDto = crud
        .create_in(&mut tx, &alice.actor, create("two"))
        .await
        .unwrap();
    drop(tx);
    assert!(fx.db.with(|state| state.notes.is_empty()));

    let mut tx = fx.db.transaction().await.unwrap();
    for title in ["one", "two"] {
        let _note: NoteDto = crud
            .create_in(&mut tx, &alice.actor, create(title))
            .await
            .unwrap();
    }
    tx.commit().await.unwrap();
    let page = crud
        .list::<NoteDto>(
            &alice.actor,
            NoteFilter {
                owner_id: Some(alice.actor.user_id),
            },
            PageRequest::default(),
        )
        .await
        .unwrap();
    assert_eq!(page.items.len(), 2);
}

#[tokio::test]
async fn duplicating_authorizes_the_source_like_a_read() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    let bob = fx.user("bob@example.com").await;
    let note = fx
        .services
        .notes
        .create(&alice.actor, create("Recipe"))
        .await
        .unwrap();
    let id = NoteId::from_uuid(note.id);

    let copy = fx.services.notes.duplicate(&alice.actor, id).await.unwrap();
    assert_ne!(copy.id, note.id);
    assert_eq!(copy.title, "Recipe");
    assert_eq!(copy.owner_id, alice.actor.user_id.as_uuid());

    let err = fx
        .services
        .notes
        .duplicate(&bob.actor, id)
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::NotFound));
    assert_eq!(fx.db.with(|state| state.notes.len()), 2);
}
