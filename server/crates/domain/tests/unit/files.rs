use domain::{
    file::{
        ContentType, FileChanges, FileName, FileSize, MAX_CONTENT_TYPE_LEN, MAX_FILE_NAME_LEN,
        MAX_FILE_SIZE, OBJECT_KEY_PREFIX, new_object_key,
    },
    object_store::{MAX_OBJECT_KEY_LEN, NewObject, Object, ObjectKey, ObjectStoreError},
};
use uuid::Uuid;

#[test]
fn name_rules() {
    assert_eq!(
        FileName::parse("  report.pdf ").unwrap().as_str(),
        "report.pdf"
    );
    assert_eq!(
        FileName::parse("Grüße 2026.txt").unwrap().to_string(),
        "Grüße 2026.txt"
    );
    assert_eq!(FileName::parse(".hidden").unwrap().as_str(), ".hidden");
    assert_eq!(FileName::parse(" ").unwrap_err().code(), "required");
    assert_eq!(
        FileName::parse(&"ä".repeat(MAX_FILE_NAME_LEN))
            .unwrap()
            .as_str()
            .chars()
            .count(),
        MAX_FILE_NAME_LEN
    );
    assert_eq!(
        FileName::parse(&"a".repeat(MAX_FILE_NAME_LEN + 1))
            .unwrap_err()
            .code(),
        "too_long"
    );
    for name in [
        "a/b",
        "..\\evil",
        ".",
        "..",
        "line\nbreak",
        "nul\0",
        "invoice\u{202E}fdp.exe",
        "isolated\u{2067}.txt",
    ] {
        assert_eq!(
            FileName::parse(name).unwrap_err().code(),
            "invalid_characters",
            "{name:?}"
        );
    }
}

#[test]
fn content_type_rules() {
    assert_eq!(
        ContentType::parse("image/PNG").unwrap().as_str(),
        "image/png"
    );
    assert_eq!(
        ContentType::parse("text/plain; charset=utf-8")
            .unwrap()
            .as_str(),
        "text/plain"
    );
    assert_eq!(
        ContentType::parse(
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
        )
        .unwrap()
        .as_str(),
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
    );
    assert_eq!(
        ContentType::parse("").unwrap().as_str(),
        ContentType::OCTET_STREAM
    );
    assert_eq!(
        ContentType::parse(" ; x=y").unwrap().as_str(),
        ContentType::OCTET_STREAM
    );
    for raw in ["text", "text/", "/plain", "text/pl ain", "té/xt", "a/b/c"] {
        assert_eq!(
            ContentType::parse(raw).unwrap_err().code(),
            "invalid_content_type",
            "{raw:?}"
        );
    }
    let long = format!("a/{}", "b".repeat(MAX_CONTENT_TYPE_LEN));
    assert!(ContentType::parse(&long).is_err());
}

#[test]
fn size_rules() {
    assert_eq!(FileSize::new(0).unwrap().bytes(), 0);
    assert_eq!(FileSize::new(MAX_FILE_SIZE).unwrap().bytes(), MAX_FILE_SIZE);
    let err = FileSize::new(MAX_FILE_SIZE + 1).unwrap_err();
    assert_eq!(err.code(), "too_large");
    assert_eq!(err.message().id(), "file-too-large");
    assert!(FileSize::new(1).unwrap() < FileSize::new(2).unwrap());
}

#[test]
fn object_keys() {
    let id = Uuid::now_v7();
    let key = ObjectKey::new("files", id);
    assert_eq!(key.as_str(), format!("files/{}", id.hyphenated()));
    assert_eq!(key.to_string(), key.as_str());
    assert_eq!(ObjectKey::parse(key.as_str()).unwrap(), key);

    let fresh = new_object_key();
    assert!(fresh.as_str().starts_with(&format!("{OBJECT_KEY_PREFIX}/")));
    assert_ne!(fresh, new_object_key());

    assert!(ObjectKey::parse("files/a_b-c.d").is_ok());
    assert!(ObjectKey::parse(&"k".repeat(MAX_OBJECT_KEY_LEN)).is_ok());
    for raw in [
        "",
        "/files/a",
        "files/a/",
        "files//a",
        "files/a b",
        "files/ä",
        "files/a?b",
        "files/%2e",
    ] {
        assert!(ObjectKey::parse(raw).is_err(), "{raw:?}");
    }
    assert!(ObjectKey::parse(&"k".repeat(MAX_OBJECT_KEY_LEN + 1)).is_err());
    assert_eq!(
        ObjectKey::parse("bad key").unwrap_err().to_string(),
        "unknown object key `bad key`"
    );
}

#[test]
fn contents_stay_out_of_debug_output() {
    let object = NewObject {
        content_type: "text/plain".to_owned(),
        length: 3,
        body: Box::pin(empty_body()),
    };
    let debug = format!("{object:?}");
    assert!(
        debug.contains("text/plain") && debug.contains("length: 3"),
        "{debug}"
    );

    let stored = Object {
        length: 3,
        body: Box::pin(empty_body()),
    };
    assert_eq!(format!("{stored:?}"), "Object { length: 3, .. }");
}

// A body without contents, written out: the domain's tests take no stream library.
fn empty_body() -> impl futures_core::Stream<Item = Result<bytes::Bytes, ObjectStoreError>> {
    struct Empty;
    impl futures_core::Stream for Empty {
        type Item = Result<bytes::Bytes, ObjectStoreError>;
        fn poll_next(
            self: std::pin::Pin<&mut Self>,
            _: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Option<Self::Item>> {
            std::task::Poll::Ready(None)
        }
    }
    Empty
}

#[test]
fn store_errors_name_their_side() {
    let body = ObjectStoreError::body(std::io::Error::other("client went away"));
    let backend = ObjectStoreError::backend(std::io::Error::other("connection refused"));
    assert_eq!(body.to_string(), "the object's contents ended early");
    assert_eq!(backend.to_string(), "object storage failed");
    assert_eq!(
        std::error::Error::source(&backend).unwrap().to_string(),
        "connection refused"
    );
}

#[test]
fn empty_changes() {
    assert!(FileChanges::default().is_empty());
    assert!(
        !FileChanges {
            name: Some(FileName::parse("a.txt").unwrap()),
        }
        .is_empty()
    );
}
