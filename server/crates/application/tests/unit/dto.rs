use std::{fs, path::PathBuf};

use application::{
    AppError, ValidationErrors,
    dto::{PermissionName, SecretInput},
    notes::dto::UpdateNoteRequest,
    pagination::{PageDto, decode_cursor, encode_cursor, page_request},
};
use domain::{
    error::{StorageError, ValidationError},
    i18n::Message,
    note::NoteChanges,
    pagination::{Cursor, NewestFirst, Page, PageRequest, PageSize},
    rbac::Permission,
};

#[test]
fn permission_names_match_the_domain() {
    for permission in Permission::ALL {
        let name = PermissionName::from(permission);
        assert_eq!(Permission::from(name), permission);
    }
}

#[test]
fn secret_input_is_redacted() {
    let secret = SecretInput::from("hunter2".to_owned());
    assert_eq!(format!("{secret:?}"), "<redacted>");
    assert_eq!(secret.0.expose(), "hunter2");
}

#[test]
fn partial_updates_keep_omitted_fields() {
    let request = UpdateNoteRequest {
        title: None,
        body: Some("new".to_owned()),
    };
    let changes = NoteChanges::try_from(request).unwrap();
    assert!(changes.title.is_none());
    assert_eq!(changes.body.unwrap().as_str(), "new");
}

#[test]
fn page_parameters_are_validated() {
    assert!(page_request(None, None, NewestFirst).is_ok());
    assert!(
        page_request(None, Some(""), NewestFirst)
            .unwrap()
            .after
            .is_none()
    );
    let errors = page_request(Some(0), Some("x"), NewestFirst).unwrap_err();
    assert_eq!(errors.fields().len(), 2);
}

#[test]
fn pages_carry_a_next_cursor() {
    let request = PageRequest::<NewestFirst>::new(PageSize::parse(Some(1)).unwrap(), None);
    let page = Page::from_rows(vec![2_u128, 1], &request, |n| {
        Cursor::from_uuid(uuid::Uuid::from_u128(*n))
    });
    let dto = PageDto::from(page);
    // Opaque base64url: the 16 bytes of the id.
    assert_eq!(dto.items, [2]);
    assert_eq!(dto.next_cursor.as_deref(), Some("AAAAAAAAAAAAAAAAAAAAAg"));
}

#[test]
fn cursors_round_trip_and_must_fit_the_order() {
    let cursor = Cursor::from_uuid(uuid::Uuid::now_v7());
    let wire = encode_cursor(&cursor);
    assert_eq!(decode_cursor(&wire, NewestFirst).unwrap(), cursor);

    // Valid base64, but no id: not a position in the default order.
    assert!(decode_cursor("AQID", NewestFirst).is_err());
    // A hyphenated id was the old format; it is not base64url.
    assert!(decode_cursor("00000000-0000-0000-0000-000000000002", NewestFirst).is_err());
    assert!(decode_cursor(&"A".repeat(10_000), NewestFirst).is_err());
    let request = page_request(Some(5), Some(&wire), NewestFirst).unwrap();
    assert_eq!(request.after, Some(cursor));
}

#[test]
fn every_error_has_a_stable_code() {
    let codes = [
        (
            AppError::Validation(ValidationErrors::new()),
            "validation_failed",
        ),
        (AppError::Unauthenticated, "unauthenticated"),
        (AppError::InvalidCredentials, "invalid_credentials"),
        (AppError::EmailNotVerified, "email_not_verified"),
        (AppError::AccountDisabled, "account_disabled"),
        (AppError::Forbidden, "forbidden"),
        (AppError::NotFound, "not_found"),
        (
            AppError::conflict("email_taken", Message::new("conflict-email-taken")),
            "email_taken",
        ),
        (AppError::InvalidToken, "invalid_token"),
        (
            AppError::from(StorageError::backend(std::io::Error::other("x"))),
            "internal_error",
        ),
    ];
    for (err, code) in codes {
        assert_eq!(err.code(), code);
    }
}

#[test]
fn validation_errors_collect_fields() {
    let mut errors = ValidationErrors::new();
    assert!(errors.is_empty());
    assert_eq!(errors.check("a", Ok::<_, ValidationError>(1)), Some(1));
    assert_eq!(
        errors.check::<u8>("b", Err(ValidationError::required())),
        None
    );
    errors.add(
        "c",
        &ValidationError::new(
            "too_long",
            Message::new("validation-too-long").arg("max", 3),
        ),
    );

    assert_eq!(errors.to_string(), "invalid fields: b, c");
    let fields: Vec<(&str, &str, &str)> = errors
        .fields()
        .iter()
        .map(|error| {
            (
                error.field.as_str(),
                error.code.as_str(),
                error.message.id(),
            )
        })
        .collect();
    assert_eq!(
        fields,
        [
            ("b", "required", "validation-required"),
            ("c", "too_long", "validation-too-long"),
        ]
    );
    assert_eq!(errors.fields()[1].message.args().len(), 1);
}

#[test]
fn limits_are_exported_for_the_frontend() {
    let dir: PathBuf = std::env::temp_dir().join(format!("export-types-{}", uuid::Uuid::now_v7()));
    fs::create_dir_all(&dir).unwrap();

    application::types::export(&dir).unwrap();

    let limits = fs::read_to_string(dir.join("limits.ts")).unwrap();
    assert_eq!(limits, application::types::limits());
    assert!(
        limits.contains(&format!(
            "export const MIN_PASSWORD_LENGTH = {};",
            domain::user::MIN_PASSWORD_LEN
        )),
        "{limits}"
    );
    fs::remove_dir_all(dir).unwrap();
}
