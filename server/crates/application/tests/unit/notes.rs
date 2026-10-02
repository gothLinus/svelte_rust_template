use application::{
    AppError,
    actor::Actor,
    notes::dto::{CreateNoteRequest, ListNotesQuery, NoteDto, NoteScope, UpdateNoteRequest},
};
use domain::{
    note::NoteId,
    rbac::{Permission, PermissionSet},
    repository::Version,
};

use crate::support::Fixture;

fn create(title: &str) -> CreateNoteRequest {
    CreateNoteRequest {
        title: title.to_owned(),
        body: Some("body".to_owned()),
    }
}

async fn note(fx: &Fixture, actor: &Actor, title: &str) -> NoteDto {
    fx.services
        .notes
        .create(actor, create(title))
        .await
        .unwrap()
}

fn id(note: &NoteDto) -> NoteId {
    NoteId::from_uuid(note.id)
}

#[tokio::test]
async fn owners_can_do_everything_with_their_notes() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;

    let created = note(&fx, &alice.actor, " Groceries ").await;
    assert_eq!(created.title, "Groceries");
    assert_eq!(created.owner_id, alice.actor.user_id.as_uuid());

    let fetched = fx
        .services
        .notes
        .get(&alice.actor, id(&created))
        .await
        .unwrap();
    assert_eq!(fetched.id, created.id);

    let updated = fx
        .services
        .notes
        .update(
            &alice.actor,
            id(&created),
            None,
            UpdateNoteRequest {
                title: None,
                body: Some("milk, eggs".to_owned()),
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.title, "Groceries");
    assert_eq!(updated.body, "milk, eggs");

    fx.services
        .notes
        .delete(&alice.actor, id(&created), None)
        .await
        .unwrap();
    let err = fx
        .services
        .notes
        .get(&alice.actor, id(&created))
        .await
        .unwrap_err();
    assert_eq!(err.code(), "not_found");
}

#[tokio::test]
async fn other_users_notes_are_invisible() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    let bob = fx.user("bob@example.com").await;
    let secret = note(&fx, &alice.actor, "Alice's").await;

    let get = fx.services.notes.get(&bob.actor, id(&secret)).await;
    let update = fx
        .services
        .notes
        .update(
            &bob.actor,
            id(&secret),
            None,
            UpdateNoteRequest {
                title: Some("pwned".to_owned()),
                body: None,
            },
        )
        .await;
    let delete = fx
        .services
        .notes
        .delete(&bob.actor, id(&secret), None)
        .await;

    // 404, not 403: bob cannot even learn that the note exists.
    for result in [get.map(drop), update.map(drop), delete] {
        assert_eq!(result.unwrap_err().code(), "not_found");
    }
    let list = fx
        .services
        .notes
        .list(&bob.actor, ListNotesQuery::default())
        .await
        .unwrap();
    assert!(list.items.is_empty());
}

#[tokio::test]
async fn managers_can_change_anyones_notes() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    let admin = fx.admin("admin@example.com").await;
    let alices = note(&fx, &alice.actor, "Alice's").await;

    let updated = fx
        .services
        .notes
        .update(
            &admin.actor,
            id(&alices),
            None,
            UpdateNoteRequest {
                title: Some("Moderated".to_owned()),
                body: None,
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.title, "Moderated");
    assert_eq!(updated.owner_id, alice.actor.user_id.as_uuid());

    fx.services
        .notes
        .delete(&admin.actor, id(&alices), None)
        .await
        .unwrap();
}

#[tokio::test]
async fn listing_everyones_notes_needs_notes_manage() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    let admin = fx.admin("admin@example.com").await;
    note(&fx, &alice.actor, "one").await;
    note(&fx, &admin.actor, "two").await;
    let all = ListNotesQuery {
        scope: Some(NoteScope::All),
        ..ListNotesQuery::default()
    };

    let err = fx
        .services
        .notes
        .list(&alice.actor, all.clone())
        .await
        .unwrap_err();
    assert_eq!(err.code(), "forbidden");

    let everything = fx.services.notes.list(&admin.actor, all).await.unwrap();
    assert_eq!(everything.items.len(), 2);
    let mine = fx
        .services
        .notes
        .list(&admin.actor, ListNotesQuery::default())
        .await
        .unwrap();
    assert_eq!(mine.items.len(), 1);
}

#[tokio::test]
async fn notes_are_paginated_newest_first() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    for n in 1..=5 {
        note(&fx, &alice.actor, &format!("note {n}")).await;
    }

    let mut titles = Vec::new();
    let mut after = None;
    loop {
        let page = fx
            .services
            .notes
            .list(
                &alice.actor,
                ListNotesQuery {
                    limit: Some(2),
                    after,
                    ..ListNotesQuery::default()
                },
            )
            .await
            .unwrap();
        titles.extend(page.items.into_iter().map(|n| n.title));
        match page.next_cursor {
            Some(cursor) => after = Some(cursor),
            None => break,
        }
    }

    assert_eq!(titles, ["note 5", "note 4", "note 3", "note 2", "note 1"]);
}

#[tokio::test]
async fn invalid_input_is_reported_per_field() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;

    let AppError::Validation(errors) = fx
        .services
        .notes
        .create(
            &alice.actor,
            CreateNoteRequest {
                title: String::new(),
                body: Some("x".repeat(10_001)),
            },
        )
        .await
        .unwrap_err()
    else {
        panic!("expected a validation error")
    };
    let fields: Vec<(&str, &str)> = errors
        .fields()
        .iter()
        .map(|e| (e.field.as_str(), e.code.as_str()))
        .collect();
    assert_eq!(fields, [("title", "required"), ("body", "too_long")]);

    let created = note(&fx, &alice.actor, "fine").await;
    let err = fx
        .services
        .notes
        .update(
            &alice.actor,
            id(&created),
            None,
            UpdateNoteRequest {
                title: Some("  ".to_owned()),
                body: None,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "validation_failed");

    let err = fx
        .services
        .notes
        .list(
            &alice.actor,
            ListNotesQuery {
                limit: Some(1000),
                ..ListNotesQuery::default()
            },
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "validation_failed");
}

#[tokio::test]
async fn permissions_are_checked_before_input() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    let read_only = Actor {
        permissions: [Permission::NotesRead].into_iter().collect(),
        ..alice.actor.clone()
    };
    let nobody = Actor {
        permissions: PermissionSet::empty(),
        ..alice.actor.clone()
    };
    let existing = note(&fx, &alice.actor, "mine").await;

    let err = fx
        .services
        .notes
        .create(&read_only, create(""))
        .await
        .unwrap_err();
    assert_eq!(err.code(), "forbidden");

    let err = fx
        .services
        .notes
        .update(
            &read_only,
            id(&existing),
            None,
            UpdateNoteRequest {
                title: Some("changed".to_owned()),
                body: None,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "forbidden");
    let err = fx
        .services
        .notes
        .delete(&read_only, id(&existing), None)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "forbidden");

    let err = fx
        .services
        .notes
        .list(&nobody, ListNotesQuery::default())
        .await
        .unwrap_err();
    assert_eq!(err.code(), "forbidden");
}

fn retitle(title: &str) -> UpdateNoteRequest {
    UpdateNoteRequest {
        title: Some(title.to_owned()),
        body: None,
    }
}

#[tokio::test]
async fn every_update_bumps_the_version_and_an_outdated_one_is_refused() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    let created = note(&fx, &alice.actor, "Draft").await;
    assert_eq!(created.version, 1);
    let read = Some(Version::new(created.version));

    let first = fx
        .services
        .notes
        .update(&alice.actor, id(&created), read, retitle("Mine"))
        .await
        .unwrap();
    assert_eq!(first.version, 2);

    // A second editor still holding version 1 does not overwrite the first one's change.
    let err = fx
        .services
        .notes
        .update(&alice.actor, id(&created), read, retitle("Theirs"))
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::Stale), "{err:?}");
    let err = fx
        .services
        .notes
        .delete(&alice.actor, id(&created), read)
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::Stale), "{err:?}");
    let kept = fx
        .services
        .notes
        .get(&alice.actor, id(&created))
        .await
        .unwrap();
    assert_eq!(kept.title, "Mine");

    fx.services
        .notes
        .delete(
            &alice.actor,
            id(&created),
            Some(Version::new(first.version)),
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn a_stale_version_does_not_reveal_a_note_the_actor_cannot_see() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    let bob = fx.user("bob@example.com").await;
    let secret = note(&fx, &alice.actor, "Secret").await;

    let err = fx
        .services
        .notes
        .update(
            &bob.actor,
            id(&secret),
            Some(Version::new(99)),
            retitle("x"),
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "not_found");
}
