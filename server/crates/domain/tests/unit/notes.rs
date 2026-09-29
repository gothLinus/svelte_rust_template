use domain::note::{MAX_NOTE_BODY_LEN, MAX_NOTE_TITLE_LEN, NoteBody, NoteChanges, NoteTitle};

#[test]
fn title_rules() {
    assert_eq!(
        NoteTitle::parse("  Groceries ").unwrap().as_str(),
        "Groceries"
    );
    assert_eq!(NoteTitle::parse("").unwrap_err().code(), "required");
    assert_eq!(
        NoteTitle::parse(&"t".repeat(MAX_NOTE_TITLE_LEN + 1))
            .unwrap_err()
            .code(),
        "too_long"
    );
    assert_eq!(
        NoteTitle::parse("two\nlines").unwrap_err().code(),
        "invalid_characters"
    );
    assert_eq!(NoteTitle::parse("Title").unwrap().to_string(), "Title");
}

#[test]
fn body_rules() {
    assert_eq!(NoteBody::parse("").unwrap().as_str(), "");
    assert_eq!(
        NoteBody::parse("line one\n\tline two\r\n")
            .unwrap()
            .as_str(),
        "line one\n\tline two"
    );
    assert_eq!(
        NoteBody::parse(&"b".repeat(MAX_NOTE_BODY_LEN + 1))
            .unwrap_err()
            .code(),
        "too_long"
    );
    assert_eq!(
        NoteBody::parse("bell\u{7}").unwrap_err().code(),
        "invalid_characters"
    );
}

#[test]
fn empty_changes() {
    assert!(NoteChanges::default().is_empty());
    assert!(
        !NoteChanges {
            title: Some(NoteTitle::parse("x").unwrap()),
            body: None,
        }
        .is_empty()
    );
}
