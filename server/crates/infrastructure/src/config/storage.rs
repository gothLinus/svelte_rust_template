//! `STORAGE_*`: the S3-compatible object store that keeps file contents (RustFS in development,
//! from `compose.yaml`), and how much each user may keep there. Off localhost the image's
//! published default credentials are refused, and a plain `http://` endpoint is worth a warning.

use domain::secret::Secret;

use super::{http::PublicOrigin, reader::Reader};

pub const DEFAULT_QUOTA_PER_USER: u64 = 1024 * 1024 * 1024;

const PUBLISHED_CREDENTIALS: &[&str] = &["rustfsadmin", "minioadmin"];

pub(super) const STORAGE_VARS: &[&str] = &[
    "STORAGE_ENDPOINT",
    "STORAGE_BUCKET",
    "STORAGE_REGION",
    "STORAGE_ACCESS_KEY",
    "STORAGE_SECRET_KEY",
    "STORAGE_QUOTA_PER_USER",
];

#[derive(Debug, Clone)]
pub struct StorageConfig {
    /// The store's S3 API, such as `http://localhost:9000`. Buckets are addressed by path
    /// (`/<bucket>/<key>`), which every S3-compatible store understands and RustFS expects unless
    /// it is given domains for virtual hosts.
    pub endpoint: PublicOrigin,
    pub bucket: String,
    pub region: String,
    pub access_key: String,
    pub secret_key: Secret,
    pub quota_per_user: Option<u64>,
}

impl Reader<'_> {
    pub(super) fn storage(&mut self) -> Option<StorageConfig> {
        let endpoint = self.required("STORAGE_ENDPOINT", |raw| {
            PublicOrigin::parse(raw).map_err(|_| {
                "must be an http:// or https:// URL without a path, e.g. http://localhost:9000"
                    .to_owned()
            })
        });
        let bucket = self.required("STORAGE_BUCKET", bucket_name);
        let region = self.parse("STORAGE_REGION", "us-east-1".to_owned(), |raw| {
            let valid = raw
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
            if valid {
                Ok(raw.to_owned())
            } else {
                Err("must be a region name such as `us-east-1`".to_owned())
            }
        });
        let access_key = self.required("STORAGE_ACCESS_KEY", |raw| Ok(raw.to_owned()));
        let secret_key = self.required("STORAGE_SECRET_KEY", |raw| Ok(Secret::new(raw)));
        let quota: u64 = self.number("STORAGE_QUOTA_PER_USER", DEFAULT_QUOTA_PER_USER);

        if !self.local {
            if access_key
                .as_deref()
                .is_some_and(|key| PUBLISHED_CREDENTIALS.contains(&key))
            {
                self.problem(
                    "STORAGE_ACCESS_KEY",
                    "is a published default; give the store credentials of its own",
                );
            }
            if secret_key
                .as_ref()
                .is_some_and(|key| PUBLISHED_CREDENTIALS.contains(&key.expose()))
            {
                self.problem(
                    "STORAGE_SECRET_KEY",
                    "is a published default; generate one with `openssl rand -hex 20`",
                );
            }
            if endpoint
                .as_ref()
                .is_some_and(|endpoint| !endpoint.is_https() && !endpoint.is_localhost())
            {
                self.warnings.push(
                    "STORAGE_ENDPOINT is plain http://: file contents travel unencrypted, \
                     which is only fine on a private network"
                        .to_owned(),
                );
            }
        }

        Some(StorageConfig {
            endpoint: endpoint?,
            bucket: bucket?,
            region,
            access_key: access_key?,
            secret_key: secret_key?,
            quota_per_user: Some(quota).filter(|quota| *quota > 0),
        })
    }
}

fn bucket_name(raw: &str) -> Result<String, String> {
    let bytes = raw.as_bytes();
    let valid = (3..=63).contains(&bytes.len())
        && bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'-' | b'.'))
        && bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        && bytes.last().is_some_and(u8::is_ascii_alphanumeric)
        && !raw.contains("..");
    if valid {
        Ok(raw.to_owned())
    } else {
        Err(
            "must be 3 to 63 lowercase letters, digits, `-` and `.`, starting and ending with a \
             letter or digit"
                .to_owned(),
        )
    }
}
