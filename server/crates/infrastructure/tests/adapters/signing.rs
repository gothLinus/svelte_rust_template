//! Signature Version 4 against the worked examples of the S3 documentation
//! (<https://docs.aws.amazon.com/AmazonS3/latest/API/sig-v4-header-based-auth.html>): the example
//! key pair, bucket and time, and the signatures AWS computed for them.

use domain::secret::Secret;
use infrastructure::object_store::{
    Signer,
    signing::{EMPTY_PAYLOAD, UNSIGNED_PAYLOAD, payload_digest},
};
use time::macros::datetime;
use url::Url;

fn signer() -> Signer {
    Signer {
        access_key: "AKIAIOSFODNN7EXAMPLE".to_owned(),
        secret_key: Secret::new("wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"),
        region: "us-east-1".to_owned(),
        service: "s3".to_owned(),
    }
}

fn signature_of(authorization: &str) -> &str {
    authorization.rsplit_once("Signature=").unwrap().1
}

#[test]
fn signs_the_get_object_example() {
    let url = Url::parse("https://examplebucket.s3.amazonaws.com/test.txt").unwrap();
    let signed = signer().sign(
        "GET",
        &url,
        &[("Range", "bytes=0-9")],
        EMPTY_PAYLOAD,
        datetime!(2013-05-24 00:00:00 UTC),
    );

    assert_eq!(signed.amz_date, "20130524T000000Z");
    assert_eq!(signed.content_sha256, EMPTY_PAYLOAD);
    assert_eq!(
        signed.authorization,
        "AWS4-HMAC-SHA256 Credential=AKIAIOSFODNN7EXAMPLE/20130524/us-east-1/s3/aws4_request, \
         SignedHeaders=host;range;x-amz-content-sha256;x-amz-date, \
         Signature=f0e8bdb87c964420e857bd35b5d6ed310bd44f0170aba48dd91039c6036bdb41"
    );
}

#[test]
fn signs_the_put_object_example() {
    let url = Url::parse("https://examplebucket.s3.amazonaws.com/test$file.text").unwrap();
    let payload = payload_digest(b"Welcome to Amazon S3.");
    assert_eq!(
        payload,
        "44ce7dd67c959e0d3524ffac1771dfbba87d2b6b4b4e99e42034a8b803f8b072"
    );

    let signed = signer().sign(
        "PUT",
        &url,
        &[
            ("Date", "Fri, 24 May 2013 00:00:00 GMT"),
            ("x-amz-storage-class", "REDUCED_REDUNDANCY"),
        ],
        &payload,
        datetime!(2013-05-24 00:00:00 UTC),
    );

    assert!(
        signed.authorization.contains(
            "SignedHeaders=date;host;x-amz-content-sha256;x-amz-date;x-amz-storage-class,"
        )
    );
    assert_eq!(
        signature_of(&signed.authorization),
        "98ad721746da40c64f1a55b78f14c238d841ea1380cd77a1b5971af0ece108bd"
    );
}

#[test]
fn signs_the_query_string_examples() {
    let at = datetime!(2013-05-24 00:00:00 UTC);

    // A subresource without a value: `?lifecycle` signs as `lifecycle=`.
    let lifecycle = Url::parse("https://examplebucket.s3.amazonaws.com/?lifecycle").unwrap();
    let signed = signer().sign("GET", &lifecycle, &[], EMPTY_PAYLOAD, at);
    assert_eq!(
        signature_of(&signed.authorization),
        "fea454ca298b7da1c68078a5d1bdbfbbe0d65c699e0f91ac7a200a0136783543"
    );

    // Parameters sign sorted by name, whatever order they were sent in.
    let list = Url::parse("https://examplebucket.s3.amazonaws.com/?prefix=J&max-keys=2").unwrap();
    let signed = signer().sign("GET", &list, &[], EMPTY_PAYLOAD, at);
    assert_eq!(
        signature_of(&signed.authorization),
        "34b48302e7b5fa45bde8084f4b7868a86f0a534bc59db6670ed5711ef69dc6f7"
    );
}

#[test]
fn signs_the_port_and_time_as_sent() {
    let url = Url::parse("http://localhost:9000/app/files/a").unwrap();
    let local = datetime!(2026-09-30 11:30:05 +02:00);
    let utc = datetime!(2026-09-30 09:30:05 UTC);

    let signed = signer().sign("PUT", &url, &[], UNSIGNED_PAYLOAD, local);

    assert_eq!(
        signed,
        signer().sign("PUT", &url, &[], UNSIGNED_PAYLOAD, utc)
    );
    assert_eq!(signed.amz_date, "20260930T093005Z");
    assert_eq!(signed.content_sha256, UNSIGNED_PAYLOAD);
    // A different port is a different host, so a different signature.
    let other_port = Url::parse("http://localhost:9001/app/files/a").unwrap();
    assert_ne!(
        signed,
        signer().sign("PUT", &other_port, &[], UNSIGNED_PAYLOAD, utc)
    );
    assert_eq!(
        signer().sign(
            "PUT",
            &url,
            &[("content-type", " text/plain ")],
            UNSIGNED_PAYLOAD,
            utc
        ),
        signer().sign(
            "PUT",
            &url,
            &[("Content-Type", "text/plain")],
            UNSIGNED_PAYLOAD,
            utc
        ),
    );
}
