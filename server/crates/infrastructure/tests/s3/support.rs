use bytes::Bytes;
use domain::{
    object_store::{ByteStream, ObjectKey, ObjectStore, ObjectStoreError},
    secret::Secret,
};
use futures_util::{StreamExt, TryStreamExt, stream};
use infrastructure::{
    config::{PublicOrigin, StorageConfig, load_dotenv},
    object_store::{S3ObjectStore, Signer, signing::EMPTY_PAYLOAD},
};
use time::OffsetDateTime;
use uuid::Uuid;

const TEST_BUCKET: &str = "tests";

pub fn config() -> StorageConfig {
    load_dotenv().unwrap();
    let var = |name: &str| {
        std::env::var(name).unwrap_or_else(|_| {
            panic!("{name} is not set: the S3 tests need RustFS (`just db-up`) and `.env`")
        })
    };
    StorageConfig {
        endpoint: PublicOrigin::parse(&var("STORAGE_ENDPOINT")).unwrap(),
        bucket: TEST_BUCKET.to_owned(),
        region: std::env::var("STORAGE_REGION").unwrap_or_else(|_| "us-east-1".to_owned()),
        access_key: var("STORAGE_ACCESS_KEY"),
        secret_key: Secret::new(var("STORAGE_SECRET_KEY")),
        quota_per_user: None,
    }
}

pub fn new_bucket_config() -> StorageConfig {
    StorageConfig {
        bucket: format!("test-{}", Uuid::now_v7().simple()),
        ..config()
    }
}

pub struct Bucket {
    pub store: S3ObjectStore,
}

impl Bucket {
    pub async fn new() -> Self {
        let store = S3ObjectStore::new(&config()).unwrap();
        store.ensure_bucket().await.unwrap();
        Self { store }
    }

    pub async fn remove(self, keys: &[&ObjectKey]) {
        for key in keys {
            self.store.delete(key).await.unwrap();
        }
    }
}

pub async fn signed(
    config: &StorageConfig,
    method: reqwest::Method,
    url: url::Url,
) -> reqwest::Response {
    let signer = Signer {
        access_key: config.access_key.clone(),
        secret_key: config.secret_key.clone(),
        region: config.region.clone(),
        service: "s3".to_owned(),
    };
    let signature = signer.sign(
        method.as_str(),
        &url,
        &[],
        EMPTY_PAYLOAD,
        OffsetDateTime::now_utc(),
    );
    infrastructure::oauth::http_client()
        .unwrap()
        .request(method, url)
        .header("x-amz-date", signature.amz_date)
        .header("x-amz-content-sha256", signature.content_sha256)
        .header("authorization", signature.authorization)
        .send()
        .await
        .unwrap()
}

pub fn key() -> ObjectKey {
    ObjectKey::new("test", Uuid::now_v7())
}

pub fn body(chunks: Vec<Result<Bytes, ObjectStoreError>>) -> ByteStream {
    Box::pin(stream::iter(chunks))
}

pub async fn read(body: ByteStream) -> Result<Vec<u8>, ObjectStoreError> {
    body.map_ok(|chunk| chunk.to_vec()).try_concat().await
}

pub async fn chunks(body: ByteStream) -> usize {
    body.count().await
}
