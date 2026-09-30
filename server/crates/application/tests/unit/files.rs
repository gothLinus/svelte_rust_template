use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use application::{
    AppError,
    account::dto::DeleteAccountRequest,
    actor::Actor,
    files::{
        UNFINISHED_UPLOAD_TTL,
        dto::{FileDto, FileScope, ListFilesQuery, UpdateFileRequest, UploadFileRequest},
    },
    maintenance::OBJECT_PURGE_RETRY,
};
use bytes::Bytes;
use domain::{
    clock::Clock,
    file::{FileId, MAX_FILE_SIZE},
    object_store::{ByteStream, ObjectKey, ObjectStore as _, ObjectStoreError},
    rbac::{Permission, PermissionSet},
};
use futures_util::{StreamExt, stream};

use crate::support::{Fixture, secret};

fn body(contents: &'static [u8]) -> ByteStream {
    let (first, rest) = contents.split_at(contents.len() / 2);
    Box::pin(stream::iter([
        Ok(Bytes::from_static(first)),
        Ok(Bytes::from_static(rest)),
    ]))
}

fn watched(read: &Arc<AtomicBool>) -> ByteStream {
    let read = Arc::clone(read);
    Box::pin(stream::once(async move {
        read.store(true, Ordering::SeqCst);
        Ok(Bytes::from_static(b"never"))
    }))
}

fn request(name: &str, content_type: &str, size: usize) -> UploadFileRequest {
    UploadFileRequest {
        name: name.to_owned(),
        content_type: content_type.to_owned(),
        size: size as u64,
    }
}

async fn upload(fx: &Fixture, actor: &Actor, name: &str, contents: &'static [u8]) -> FileDto {
    fx.services
        .files
        .upload(
            actor,
            request(name, "text/plain", contents.len()),
            body(contents),
        )
        .await
        .unwrap()
}

fn id(file: &FileDto) -> FileId {
    FileId::from_uuid(file.id)
}

fn key_of(fx: &Fixture, file: &FileDto) -> ObjectKey {
    fx.db
        .with(|state| state.files[&id(file)].object_key().clone())
}

async fn download(fx: &Fixture, actor: &Actor, file: &FileDto) -> Result<Vec<u8>, AppError> {
    let download = fx.services.files.download(actor, id(file)).await?;
    let chunks: Vec<Result<Bytes, ObjectStoreError>> = download.body.collect().await;
    Ok(chunks
        .into_iter()
        .flat_map(|chunk| chunk.unwrap().to_vec())
        .collect())
}

#[tokio::test]
async fn an_upload_stores_the_contents_and_the_row() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;

    let file = fx
        .services
        .files
        .upload(
            &alice.actor,
            request("  notes.TXT ", "Text/Plain; charset=utf-8", 11),
            body(b"hello world"),
        )
        .await
        .unwrap();

    assert_eq!(file.name, "notes.TXT");
    assert_eq!(file.content_type, "text/plain");
    assert_eq!(file.size, 11);
    assert_eq!(file.owner_id, alice.actor.user_id.as_uuid());
    let key = key_of(&fx, &file);
    assert!(key.as_str().starts_with("files/"));
    assert_eq!(fx.objects.contents(&key).unwrap(), b"hello world");
    assert!(fx.db.with(|state| state.object_deletions.is_empty()));

    let download = fx
        .services
        .files
        .download(&alice.actor, id(&file))
        .await
        .unwrap();
    assert_eq!(download.name, "notes.TXT");
    assert_eq!(download.content_type, "text/plain");
    assert_eq!(download.size, 11);
    assert_eq!(download_of(download.body).await, b"hello world");
}

async fn download_of(body: ByteStream) -> Vec<u8> {
    body.map(|chunk| chunk.unwrap().to_vec()).concat().await
}

#[tokio::test]
async fn an_upload_without_a_type_is_octet_stream() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;

    let file = fx
        .services
        .files
        .upload(&alice.actor, request("data.bin", "", 3), body(b"abc"))
        .await
        .unwrap();

    assert_eq!(file.content_type, "application/octet-stream");
}

#[tokio::test]
async fn an_invalid_upload_is_refused_before_its_body_is_read() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    let read = Arc::new(AtomicBool::new(false));

    let err = fx
        .services
        .files
        .upload(
            &alice.actor,
            request(
                "../etc",
                "not a type",
                usize::try_from(MAX_FILE_SIZE).unwrap() + 1,
            ),
            watched(&read),
        )
        .await
        .unwrap_err();

    let AppError::Validation(errors) = err else {
        panic!("expected field errors, got {err:?}");
    };
    let fields: Vec<(&str, &str)> = errors
        .fields()
        .iter()
        .map(|error| (error.field.as_str(), error.code.as_str()))
        .collect();
    assert_eq!(
        fields,
        [
            ("name", "invalid_characters"),
            ("contentType", "invalid_content_type"),
            ("file", "too_large"),
        ]
    );
    assert_eq!(
        fx.say(&errors.fields()[2].message),
        fx.say(&domain::i18n::Message::new("file-too-large").arg("max", 25))
    );
    assert!(!read.load(Ordering::SeqCst));
    assert!(fx.objects.keys().is_empty());
    assert!(fx.db.with(|state| state.object_deletions.is_empty()));

    let err = fx
        .services
        .files
        .upload(&alice.actor, request("   ", "", 0), body(b""))
        .await
        .unwrap_err();
    let AppError::Validation(errors) = err else {
        panic!("expected field errors, got {err:?}");
    };
    assert_eq!(errors.fields()[0].field, "name");
    assert_eq!(errors.fields()[0].code, "required");
}

#[tokio::test]
async fn uploading_needs_files_write_before_anything_else() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    let read_only = Actor {
        permissions: [Permission::FilesRead].into_iter().collect(),
        ..alice.actor.clone()
    };
    let read = Arc::new(AtomicBool::new(false));

    let err = fx
        .services
        .files
        .upload(&read_only, request("", "", 5), watched(&read))
        .await
        .unwrap_err();

    assert!(matches!(err, AppError::Forbidden), "{err:?}");
    assert!(!read.load(Ordering::SeqCst));
    assert!(fx.objects.keys().is_empty());
}

#[tokio::test]
async fn the_quota_counts_what_is_stored() {
    let fx = Fixture::with(|settings| settings.file_quota = Some(10));
    let alice = fx.user("alice@example.com").await;
    let bob = fx.user("bob@example.com").await;
    upload(&fx, &alice.actor, "six.txt", b"123456").await;
    let read = Arc::new(AtomicBool::new(false));

    let err = fx
        .services
        .files
        .upload(&alice.actor, request("five.txt", "", 5), watched(&read))
        .await
        .unwrap_err();

    let AppError::Conflict { code, message } = &err else {
        panic!("expected a conflict, got {err:?}");
    };
    assert_eq!(*code, "file_quota_exceeded");
    assert_eq!(fx.say(message), fx.text("conflict-file-quota-exceeded"));
    assert!(!read.load(Ordering::SeqCst));
    upload(&fx, &alice.actor, "four.txt", b"1234").await;
    upload(&fx, &bob.actor, "ten.txt", b"1234567890").await;

    let usage = fx.services.files.usage(&alice.actor).await.unwrap();
    assert_eq!(usage.used_bytes, 10);
    assert_eq!(usage.quota_bytes, Some(10));
}

#[tokio::test]
async fn uploads_side_by_side_cannot_pass_the_quota_together() {
    let fx = Fixture::with(|settings| settings.file_quota = Some(10));
    let alice = fx.user("alice@example.com").await;
    let (release, released) = tokio::sync::oneshot::channel::<()>();
    // The first body arrives only once the second upload is stored, so both pass the check made
    // before their bodies are read.
    let held: ByteStream = Box::pin(stream::once(async move {
        released.await.unwrap();
        Ok(Bytes::from_static(b"123456"))
    }));
    let first = fx
        .services
        .files
        .upload(&alice.actor, request("first.txt", "text/plain", 6), held);
    let second = async {
        let file = upload(&fx, &alice.actor, "second.txt", b"123456").await;
        release.send(()).unwrap();
        file
    };

    let (first, second) = tokio::join!(first, second);

    let err = first.unwrap_err();
    let AppError::Conflict { code, .. } = &err else {
        panic!("expected a conflict, got {err:?}");
    };
    assert_eq!(*code, "file_quota_exceeded");
    assert_eq!(fx.objects.keys(), [key_of(&fx, &second)]);
    assert!(fx.db.with(|state| state.object_deletions.is_empty()));
    let usage = fx.services.files.usage(&alice.actor).await.unwrap();
    assert_eq!(usage.used_bytes, 6);
}

#[tokio::test]
async fn usage_without_a_quota_and_without_permission() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    upload(&fx, &alice.actor, "a.txt", b"abc").await;

    let usage = fx.services.files.usage(&alice.actor).await.unwrap();
    assert_eq!(usage.used_bytes, 3);
    assert_eq!(usage.quota_bytes, None);

    let nobody = Actor {
        permissions: PermissionSet::empty(),
        ..alice.actor.clone()
    };
    let err = fx.services.files.usage(&nobody).await.unwrap_err();
    assert!(matches!(err, AppError::Forbidden), "{err:?}");
}

#[tokio::test]
async fn an_upload_that_breaks_off_leaves_its_key_queued() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    let broken: ByteStream = Box::pin(stream::iter([
        Ok(Bytes::from_static(b"half")),
        Err(ObjectStoreError::body(std::io::Error::other(
            "client went away",
        ))),
    ]));

    let err = fx
        .services
        .files
        .upload(&alice.actor, request("big.bin", "", 8), broken)
        .await
        .unwrap_err();

    assert!(matches!(err, AppError::IncompleteUpload), "{err:?}");
    assert_eq!(err.code(), "incomplete_upload");
    assert!(fx.db.with(|state| state.files.is_empty()));
    let queued: Vec<_> = fx.db.with(|state| {
        state
            .object_deletions
            .iter()
            .map(|(key, due)| (key.clone(), *due))
            .collect()
    });
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].1, Some(fx.clock.now() + UNFINISHED_UPLOAD_TTL));

    // Not before its time: an upload that slow might still be running.
    let early = fx.services.maintenance.delete_expired().await.unwrap();
    assert_eq!(early.objects, 0);
    fx.clock.advance(UNFINISHED_UPLOAD_TTL);
    let cleanup = fx.services.maintenance.delete_expired().await.unwrap();
    assert_eq!(cleanup.objects, 1);
    assert!(fx.db.with(|state| state.object_deletions.is_empty()));
}

#[tokio::test]
async fn an_upload_the_store_refuses_is_internal() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    fx.objects.set_down(true);

    let err = fx
        .services
        .files
        .upload(&alice.actor, request("a.txt", "", 3), body(b"abc"))
        .await
        .unwrap_err();

    assert!(matches!(err, AppError::Internal(_)), "{err:?}");
    assert!(fx.db.with(|state| state.files.is_empty()));
    assert_eq!(fx.db.with(|state| state.object_deletions.len()), 1);
}

#[tokio::test]
async fn owners_rename_and_delete_their_files() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    let file = upload(&fx, &alice.actor, "draft.txt", b"text").await;
    let key = key_of(&fx, &file);

    let renamed = fx
        .services
        .files
        .update(
            &alice.actor,
            id(&file),
            UpdateFileRequest {
                name: Some(" final.txt ".to_owned()),
            },
        )
        .await
        .unwrap();
    assert_eq!(renamed.name, "final.txt");
    assert_eq!(renamed.size, 4);
    let unchanged = fx
        .services
        .files
        .update(&alice.actor, id(&file), UpdateFileRequest { name: None })
        .await
        .unwrap();
    assert_eq!(unchanged.name, "final.txt");
    let err = fx
        .services
        .files
        .update(
            &alice.actor,
            id(&file),
            UpdateFileRequest {
                name: Some("a/b".to_owned()),
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::Validation(_)), "{err:?}");

    fx.services
        .files
        .delete(&alice.actor, id(&file))
        .await
        .unwrap();

    assert!(fx.objects.contents(&key).is_none());
    assert!(fx.db.with(|state| state.object_deletions.is_empty()));
    let err = fx
        .services
        .files
        .get(&alice.actor, id(&file))
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::NotFound), "{err:?}");
}

#[tokio::test]
async fn a_delete_while_the_store_is_down_is_finished_by_maintenance() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    let first = upload(&fx, &alice.actor, "a.txt", b"a").await;
    let second = upload(&fx, &alice.actor, "b.txt", b"b").await;
    let (first_key, second_key) = (key_of(&fx, &first), key_of(&fx, &second));
    fx.objects.set_down(true);

    fx.services
        .files
        .delete(&alice.actor, id(&first))
        .await
        .unwrap();
    fx.services
        .files
        .delete(&alice.actor, id(&second))
        .await
        .unwrap();
    assert!(fx.db.with(|state| state.files.is_empty()));
    assert_eq!(fx.objects.keys().len(), 2);

    let cleanup = fx.services.maintenance.delete_expired().await.unwrap();
    assert_eq!(cleanup.objects, 0);
    let due: Vec<_> = fx
        .db
        .with(|state| state.object_deletions.values().copied().collect());
    assert!(due.contains(&Some(fx.clock.now() + OBJECT_PURGE_RETRY)));
    assert!(due.contains(&None));

    fx.objects.set_down(false);
    let cleanup = fx.services.maintenance.delete_expired().await.unwrap();
    assert_eq!(cleanup.objects, 1);
    fx.clock.advance(OBJECT_PURGE_RETRY);
    let cleanup = fx.services.maintenance.delete_expired().await.unwrap();
    assert_eq!(cleanup.objects, 1);

    assert!(fx.objects.contents(&first_key).is_none());
    assert!(fx.objects.contents(&second_key).is_none());
    assert!(fx.db.with(|state| state.object_deletions.is_empty()));
}

#[tokio::test]
async fn deleting_the_account_queues_its_files_contents() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    let bob = fx.user("bob@example.com").await;
    let mine = upload(&fx, &alice.actor, "mine.txt", b"alice's").await;
    let theirs = upload(&fx, &bob.actor, "theirs.txt", b"bob's").await;
    let (mine, theirs) = (key_of(&fx, &mine), key_of(&fx, &theirs));

    fx.services
        .account
        .delete_account(
            &alice.actor,
            DeleteAccountRequest {
                password: Some(secret(crate::support::PASSWORD)),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        fx.db
            .with(|state| state.object_deletions.keys().cloned().collect::<Vec<_>>()),
        std::slice::from_ref(&mine)
    );

    let cleanup = fx.services.maintenance.delete_expired().await.unwrap();

    assert_eq!(cleanup.objects, 1);
    assert!(fx.objects.contents(&mine).is_none());
    assert!(fx.objects.contents(&theirs).is_some());
}

#[tokio::test]
async fn other_users_files_are_invisible_unless_managed() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    let bob = fx.user("bob@example.com").await;
    let admin = fx.admin("admin@example.com").await;
    let file = upload(&fx, &alice.actor, "private.txt", b"secret").await;

    for err in [
        fx.services
            .files
            .get(&bob.actor, id(&file))
            .await
            .unwrap_err(),
        download(&fx, &bob.actor, &file).await.unwrap_err(),
        fx.services
            .files
            .delete(&bob.actor, id(&file))
            .await
            .unwrap_err(),
    ] {
        assert!(matches!(err, AppError::NotFound), "{err:?}");
    }
    let everyone = ListFilesQuery {
        scope: Some(FileScope::All),
        ..ListFilesQuery::default()
    };
    let err = fx
        .services
        .files
        .list(&bob.actor, everyone.clone())
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::Forbidden), "{err:?}");
    assert!(
        fx.services
            .files
            .list(&bob.actor, ListFilesQuery::default())
            .await
            .unwrap()
            .items
            .is_empty()
    );

    let all = fx
        .services
        .files
        .list(&admin.actor, everyone)
        .await
        .unwrap();
    assert_eq!(all.items.len(), 1);
    assert_eq!(download(&fx, &admin.actor, &file).await.unwrap(), b"secret");
    fx.services
        .files
        .delete(&admin.actor, id(&file))
        .await
        .unwrap();
}

#[tokio::test]
async fn read_only_holders_download_but_do_not_change() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    let file = upload(&fx, &alice.actor, "a.txt", b"abc").await;
    let read_only = Actor {
        permissions: [Permission::FilesRead].into_iter().collect(),
        ..alice.actor.clone()
    };

    assert_eq!(download(&fx, &read_only, &file).await.unwrap(), b"abc");
    let err = fx
        .services
        .files
        .delete(&read_only, id(&file))
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::Forbidden), "{err:?}");
    let err = fx
        .services
        .files
        .update(
            &read_only,
            id(&file),
            UpdateFileRequest {
                name: Some("b.txt".to_owned()),
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(err, AppError::Forbidden), "{err:?}");
}

#[tokio::test]
async fn lists_newest_first_in_pages() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    for name in ["1.txt", "2.txt", "3.txt"] {
        upload(&fx, &alice.actor, name, b"x").await;
    }

    let first = fx
        .services
        .files
        .list(
            &alice.actor,
            ListFilesQuery {
                limit: Some(2),
                ..ListFilesQuery::default()
            },
        )
        .await
        .unwrap();
    let names: Vec<&str> = first.items.iter().map(|file| file.name.as_str()).collect();
    assert_eq!(names, ["3.txt", "2.txt"]);

    let rest = fx
        .services
        .files
        .list(
            &alice.actor,
            ListFilesQuery {
                limit: Some(2),
                after: first.next_cursor,
                ..ListFilesQuery::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(rest.items.len(), 1);
    assert_eq!(rest.items[0].name, "1.txt");
    assert!(rest.next_cursor.is_none());
}

#[tokio::test]
async fn a_file_whose_contents_are_lost_is_internal() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    let file = upload(&fx, &alice.actor, "a.txt", b"abc").await;
    let key = key_of(&fx, &file);
    fx.objects.delete(&key).await.unwrap();

    let err = download(&fx, &alice.actor, &file).await.unwrap_err();

    assert!(matches!(err, AppError::Internal(_)), "{err:?}");
    assert!(format!("{err:?}").contains(key.as_str()));
}

#[tokio::test]
async fn the_export_lists_the_users_files() {
    let fx = Fixture::new();
    let alice = fx.user("alice@example.com").await;
    let bob = fx.user("bob@example.com").await;
    upload(&fx, &alice.actor, "report.pdf", b"%PDF").await;
    upload(&fx, &bob.actor, "bobs.txt", b"no").await;

    let export = fx.services.account.export(&alice.actor).await.unwrap();

    let files = export["files"].as_array().unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["name"], "report.pdf");
    assert_eq!(files[0]["content_type"], "text/plain");
    assert_eq!(files[0]["size"], 4);
    assert!(files[0].get("object_key").is_none());
}
