use domain::{
    clock::Clock,
    mail::{Mail, Mailer},
    user::Email,
};
use infrastructure::testing::{ManualClock, RecordingMailer};
use time::{Duration, macros::datetime};

#[test]
fn the_manual_clock_moves_only_when_told() {
    let clock = ManualClock::new(datetime!(2026-01-01 00:00:00.123_456_789 UTC));
    assert_eq!(clock.now(), datetime!(2026-01-01 00:00:00.123_456 UTC));

    clock.advance(Duration::hours(1));
    assert_eq!(clock.now(), datetime!(2026-01-01 01:00:00.123_456 UTC));

    clock.set(datetime!(2030-01-01 00:00 UTC));
    assert_eq!(clock.clone().now(), datetime!(2030-01-01 00:00 UTC));
    assert!(ManualClock::starting_now().now() > datetime!(2026-01-01 00:00 UTC));
}

#[tokio::test]
async fn the_recording_mailer_keeps_everything() {
    let mailer = RecordingMailer::new();
    for to in ["a@example.com", "b@example.com", "a@example.com"] {
        mailer
            .send(Mail {
                to: Email::parse(to).unwrap(),
                template: "test".to_owned(),
                subject: format!("to {to}"),
                body: String::new(),
                valid_for: None,
            })
            .await
            .unwrap();
    }

    assert_eq!(mailer.sent().len(), 3);
    assert!(mailer.last_to("b@example.com").is_some());
    assert!(mailer.last_to("c@example.com").is_none());
    assert_eq!(mailer.take().len(), 3);
    assert!(mailer.sent().is_empty());
}

#[tokio::test]
async fn the_memory_object_store_streams_and_checks_lengths() {
    use bytes::Bytes;
    use domain::object_store::{NewObject, ObjectKey, ObjectStore, ObjectStoreError};
    use futures_util::{StreamExt, stream};
    use infrastructure::testing::MemoryObjectStore;

    let store = MemoryObjectStore::new();
    let key = ObjectKey::new("test", uuid::Uuid::now_v7());
    let contents = vec![1u8; 40 * 1024];
    let object = |length: u64, contents: Vec<u8>| NewObject {
        content_type: "application/octet-stream".to_owned(),
        length,
        body: Box::pin(stream::iter([Ok(Bytes::from(contents))])),
    };

    store
        .put(&key, object(40 * 1024, contents.clone()))
        .await
        .unwrap();
    assert_eq!(store.keys(), std::slice::from_ref(&key));
    let (content_type, stored) = store.object(&key).unwrap();
    assert_eq!(content_type, "application/octet-stream");
    assert_eq!(stored, contents);

    let read = store.get(&key).await.unwrap().unwrap();
    assert_eq!(read.length, 40 * 1024);
    let chunks: Vec<_> = read.body.collect().await;
    assert_eq!(chunks.len(), 3);

    let other = ObjectKey::new("test", uuid::Uuid::now_v7());
    let err = store.put(&other, object(1, vec![1, 2])).await.unwrap_err();
    assert!(matches!(err, ObjectStoreError::Body(_)));
    assert!(store.get(&other).await.unwrap().is_none());

    store.set_down(true);
    assert!(matches!(
        store.get(&key).await.unwrap_err(),
        ObjectStoreError::Backend(_)
    ));
    assert!(store.delete(&key).await.is_err());
    assert!(store.put(&other, object(0, Vec::new())).await.is_err());
    store.set_down(false);

    store.delete(&key).await.unwrap();
    store.insert(other.clone(), "text/plain", "direct");
    assert_eq!(store.keys(), [other]);
}
