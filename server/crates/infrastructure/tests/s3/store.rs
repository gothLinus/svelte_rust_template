use bytes::Bytes;
use domain::object_store::{NewObject, ObjectStore, ObjectStoreError};
use infrastructure::{
    config::PublicOrigin,
    object_store::{BucketStatus, S3ObjectStore},
};

use crate::support::{Bucket, body, chunks, config, key, new_bucket_config, read};

fn object(content_type: &str, chunks: &[&'static [u8]]) -> NewObject {
    let length = chunks.iter().map(|chunk| chunk.len() as u64).sum();
    NewObject {
        content_type: content_type.to_owned(),
        length,
        body: body(
            chunks
                .iter()
                .map(|chunk| Ok(Bytes::from_static(chunk)))
                .collect(),
        ),
    }
}

#[tokio::test]
async fn creates_a_missing_bucket_once() {
    let config = new_bucket_config();
    let store = S3ObjectStore::new(&config).unwrap();

    assert_eq!(store.ensure_bucket().await.unwrap(), BucketStatus::Created);
    assert_eq!(store.ensure_bucket().await.unwrap(), BucketStatus::Existed);

    let status =
        crate::support::signed(&config, reqwest::Method::DELETE, store.bucket_url().clone())
            .await
            .status();
    assert!(status.is_success());
}

#[tokio::test]
async fn ping_needs_the_bucket_and_the_credentials() {
    let bucket = Bucket::new().await;
    bucket.store.ping().await.unwrap();

    let missing = S3ObjectStore::new(&new_bucket_config()).unwrap();
    assert!(matches!(
        missing.ping().await.unwrap_err(),
        ObjectStoreError::Backend(_)
    ));

    let mut wrong = config();
    wrong.secret_key = domain::secret::Secret::new("not-the-secret-key");
    let refused = S3ObjectStore::new(&wrong).unwrap();
    assert!(matches!(
        refused.ping().await.unwrap_err(),
        ObjectStoreError::Backend(_)
    ));
    bucket.remove(&[]).await;
}

#[tokio::test]
async fn stores_reads_and_deletes_an_object() {
    let bucket = Bucket::new().await;
    let key = key();
    // A megabyte and a bit, sent in pieces: more than one read's worth coming back.
    let big: &'static [u8] = Box::leak(vec![7u8; 1024 * 1024].into_boxed_slice());

    bucket
        .store
        .put(&key, object("text/plain", &[b"hello ", big, b" world"]))
        .await
        .unwrap();

    let stored = bucket.store.get(&key).await.unwrap().unwrap();
    assert_eq!(stored.length, 1024 * 1024 + 12);
    let contents = read(stored.body).await.unwrap();
    assert_eq!(contents.len(), 1024 * 1024 + 12);
    assert!(contents.starts_with(b"hello \x07"));
    assert!(contents.ends_with(b"\x07 world"));
    let again = bucket.store.get(&key).await.unwrap().unwrap();
    assert!(chunks(again.body).await > 1);

    bucket
        .store
        .put(&key, object("text/plain", &[b"again"]))
        .await
        .unwrap();
    let stored = bucket.store.get(&key).await.unwrap().unwrap();
    assert_eq!(read(stored.body).await.unwrap(), b"again");

    bucket.store.delete(&key).await.unwrap();
    assert!(bucket.store.get(&key).await.unwrap().is_none());
    bucket.remove(&[&key]).await;
}

#[tokio::test]
async fn stores_empty_objects() {
    let bucket = Bucket::new().await;
    let key = key();

    bucket
        .store
        .put(&key, object("application/octet-stream", &[]))
        .await
        .unwrap();
    let stored = bucket.store.get(&key).await.unwrap().unwrap();
    assert_eq!(stored.length, 0);
    assert!(read(stored.body).await.unwrap().is_empty());

    bucket.remove(&[&key]).await;
}

#[tokio::test]
async fn a_missing_object_is_none() {
    let bucket = Bucket::new().await;

    assert!(bucket.store.get(&key()).await.unwrap().is_none());

    bucket.remove(&[]).await;
}

#[tokio::test]
async fn refuses_a_body_of_another_length_and_stores_nothing() {
    let bucket = Bucket::new().await;
    let short = key();
    let long = key();

    let mut too_short = object("text/plain", &[b"four"]);
    too_short.length = 10;
    let err = bucket.store.put(&short, too_short).await.unwrap_err();
    assert!(matches!(err, ObjectStoreError::Body(_)), "{err:?}");

    let mut too_long = object("text/plain", &[b"four", b"more"]);
    too_long.length = 4;
    let err = bucket.store.put(&long, too_long).await.unwrap_err();
    assert!(matches!(err, ObjectStoreError::Body(_)), "{err:?}");

    assert!(bucket.store.get(&short).await.unwrap().is_none());
    assert!(bucket.store.get(&long).await.unwrap().is_none());
    bucket.remove(&[]).await;
}

#[tokio::test]
async fn a_body_that_fails_is_the_bodys_fault() {
    let bucket = Bucket::new().await;
    let key = key();

    let broken = NewObject {
        content_type: "text/plain".to_owned(),
        length: 10,
        body: body(vec![
            Ok(Bytes::from_static(b"12345")),
            Err(ObjectStoreError::body(std::io::Error::other(
                "client went away",
            ))),
        ]),
    };
    let err = bucket.store.put(&key, broken).await.unwrap_err();

    assert!(matches!(err, ObjectStoreError::Body(_)), "{err:?}");
    assert!(bucket.store.get(&key).await.unwrap().is_none());
    bucket.remove(&[]).await;
}

#[tokio::test]
async fn wrong_credentials_are_refused_at_the_bucket_check() {
    let mut config = config();
    config.secret_key = domain::secret::Secret::new("not-the-secret-key");
    let store = S3ObjectStore::new(&config).unwrap();

    let err = store.ensure_bucket().await.unwrap_err();

    assert!(matches!(err, ObjectStoreError::Backend(_)), "{err:?}");
    assert!(err.to_string().contains("object storage failed"));
    let source = std::error::Error::source(&err).unwrap().to_string();
    assert!(source.contains("403"), "{source}");
}

#[tokio::test]
async fn a_store_that_cannot_be_reached_is_a_backend_error() {
    let mut config = config();
    // Nothing listens on the discard port.
    config.endpoint = PublicOrigin::parse("http://127.0.0.1:9").unwrap();
    let store = S3ObjectStore::new(&config).unwrap();
    let key = key();

    assert!(matches!(
        store.ensure_bucket().await.unwrap_err(),
        ObjectStoreError::Backend(_)
    ));
    assert!(matches!(
        store.get(&key).await.unwrap_err(),
        ObjectStoreError::Backend(_)
    ));
    assert!(matches!(
        store.delete(&key).await.unwrap_err(),
        ObjectStoreError::Backend(_)
    ));
    assert!(matches!(
        store.ping().await.unwrap_err(),
        ObjectStoreError::Backend(_)
    ));
    assert!(matches!(
        store
            .put(&key, object("text/plain", &[b"x"]))
            .await
            .unwrap_err(),
        ObjectStoreError::Backend(_)
    ));
}

#[tokio::test]
async fn refusals_carry_the_s3_error_code() {
    let config = new_bucket_config();
    let store = S3ObjectStore::new(&config).unwrap();

    let err = store
        .put(&key(), object("text/plain", &[b"x"]))
        .await
        .unwrap_err();

    let source = std::error::Error::source(&err).unwrap().to_string();
    assert!(
        source.contains("404") && source.contains("NoSuchBucket"),
        "{source}"
    );
}
