//! AWS Signature Version 4, the authentication of the S3 API: each request carries a signature
//! over its method, path, query, chosen headers and a digest of its body, made with a key derived
//! from the secret key, the date, the region and the service. The secret itself never travels.
//!
//! [`Signer::sign`] is a pure function of the request and the time, so it is tested against the
//! examples AWS publishes. Bodies that stream are signed as `UNSIGNED-PAYLOAD`: their digest is
//! not known before they are sent, and the connection's TLS protects them. Reference:
//! <https://docs.aws.amazon.com/AmazonS3/latest/API/sig-v4-header-based-auth.html>.

use std::fmt::Write as _;

use domain::secret::Secret;
use ring::{digest, hmac};
use time::{OffsetDateTime, UtcOffset};
use url::Url;

pub const UNSIGNED_PAYLOAD: &str = "UNSIGNED-PAYLOAD";

pub const EMPTY_PAYLOAD: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

const ALGORITHM: &str = "AWS4-HMAC-SHA256";

#[derive(Debug, Clone)]
pub struct Signer {
    pub access_key: String,
    pub secret_key: Secret,
    pub region: String,
    pub service: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature {
    pub amz_date: String,
    pub content_sha256: String,
    pub authorization: String,
}

impl Signer {
    /// Signs a request to `url` at `at`. `headers` are the ones to sign besides `host`,
    /// `x-amz-content-sha256` and `x-amz-date`, which are always signed, as they will be sent;
    /// names in any case. `payload` is the body's SHA-256 in hex, [`EMPTY_PAYLOAD`] or
    /// [`UNSIGNED_PAYLOAD`].
    pub fn sign(
        &self,
        method: &str,
        url: &Url,
        headers: &[(&str, &str)],
        payload: &str,
        at: OffsetDateTime,
    ) -> Signature {
        let at = at.to_offset(UtcOffset::UTC);
        let date = format!("{:04}{:02}{:02}", at.year(), u8::from(at.month()), at.day());
        let amz_date = format!(
            "{date}T{:02}{:02}{:02}Z",
            at.hour(),
            at.minute(),
            at.second()
        );

        let mut signed: Vec<(String, String)> = headers
            .iter()
            .map(|(name, value)| (name.to_ascii_lowercase(), canonical_value(value)))
            .collect();
        signed.push(("host".to_owned(), host(url)));
        signed.push(("x-amz-content-sha256".to_owned(), payload.to_owned()));
        signed.push(("x-amz-date".to_owned(), amz_date.clone()));
        signed.sort();
        let names = signed
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>()
            .join(";");
        let canonical_headers = signed.iter().fold(String::new(), |mut out, (name, value)| {
            let _infallible = writeln!(out, "{name}:{value}");
            out
        });

        let canonical_request = format!(
            "{method}\n{}\n{}\n{canonical_headers}\n{names}\n{payload}",
            canonical_path(url),
            canonical_query(url),
        );
        let scope = format!("{date}/{}/{}/aws4_request", self.region, self.service);
        let string_to_sign = format!(
            "{ALGORITHM}\n{amz_date}\n{scope}\n{}",
            hex(digest::digest(&digest::SHA256, canonical_request.as_bytes()).as_ref())
        );

        let key = [
            date.as_str(),
            self.region.as_str(),
            self.service.as_str(),
            "aws4_request",
        ]
        .iter()
        .fold(
            format!("AWS4{}", self.secret_key.expose()).into_bytes(),
            |key, part| mac(&key, part.as_bytes()),
        );
        let signature = hex(&mac(&key, string_to_sign.as_bytes()));

        Signature {
            authorization: format!(
                "{ALGORITHM} Credential={}/{scope}, SignedHeaders={names}, Signature={signature}",
                self.access_key
            ),
            amz_date,
            content_sha256: payload.to_owned(),
        }
    }
}

pub fn payload_digest(body: &[u8]) -> String {
    hex(digest::digest(&digest::SHA256, body).as_ref())
}

fn mac(key: &[u8], data: &[u8]) -> Vec<u8> {
    hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, key), data)
        .as_ref()
        .to_vec()
}

fn hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut out, byte| {
            let _infallible = write!(out, "{byte:02x}");
            out
        })
}

/// `Host` as the client sends it: the port only when it is not the scheme's default.
fn host(url: &Url) -> String {
    let host = url.host_str().unwrap_or_default();
    match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_owned(),
    }
}

/// The path with each segment percent-encoded once, as S3 signs it. `Url` leaves some characters
/// (`$`) as they are and encodes others (spaces), so each segment is decoded first.
fn canonical_path(url: &Url) -> String {
    let path = url
        .path()
        .split('/')
        .map(|segment| encode_bytes(&decode(segment)))
        .collect::<Vec<_>>()
        .join("/");
    if path.is_empty() {
        "/".to_owned()
    } else {
        path
    }
}

fn decode(segment: &str) -> Vec<u8> {
    let bytes = segment.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = bytes
            .get(i + 1..i + 3)
            .and_then(|pair| std::str::from_utf8(pair).ok())
            .and_then(|pair| u8::from_str_radix(pair, 16).ok());
        match (bytes[i], hex) {
            (b'%', Some(byte)) => {
                out.push(byte);
                i += 3;
            }
            (byte, _) => {
                out.push(byte);
                i += 1;
            }
        }
    }
    out
}

fn canonical_query(url: &Url) -> String {
    let mut pairs: Vec<(String, String)> = url
        .query_pairs()
        .map(|(name, value)| (encode(&name), encode(&value)))
        .collect();
    pairs.sort();
    pairs
        .iter()
        .map(|(name, value)| format!("{name}={value}"))
        .collect::<Vec<_>>()
        .join("&")
}

fn encode(raw: &str) -> String {
    encode_bytes(raw.as_bytes())
}

fn encode_bytes(raw: &[u8]) -> String {
    raw.iter().fold(String::new(), |mut out, &byte| {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'~') {
            out.push(char::from(byte));
        } else {
            let _infallible = write!(out, "%{byte:02X}");
        }
        out
    })
}

fn canonical_value(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}
