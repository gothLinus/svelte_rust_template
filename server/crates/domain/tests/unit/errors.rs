use std::{fmt, io};

use domain::{
    clock::truncate_to_micros,
    error::{ErrorChain, StorageError, ValidationError},
    i18n::Message,
};
use time::macros::datetime;

#[test]
fn validation_error_carries_code_and_message() {
    let message = Message::new("validation-too-long").arg("max", 5);
    let err = ValidationError::new("too_long", message.clone());
    assert_eq!(err.code(), "too_long");
    assert_eq!(err.message(), &message);
    // For logs: the code and which message would be shown, never a sentence.
    assert_eq!(err.to_string(), "too_long (validation-too-long)");
    assert_eq!(ValidationError::required().code(), "required");
    assert_eq!(
        ValidationError::required().message().id(),
        "validation-required"
    );
}

#[test]
fn unique_violation_is_matched_by_constraint_name() {
    let err = StorageError::UniqueViolation {
        constraint: "users_email_lower_key".to_owned(),
    };
    assert!(err.is_unique_violation("users_email_lower_key"));
    assert!(!err.is_unique_violation("other"));
    assert!(
        !StorageError::ForeignKeyViolation {
            constraint: "users_email_lower_key".to_owned()
        }
        .is_unique_violation("users_email_lower_key")
    );
}

#[derive(Debug)]
struct Outer(io::Error);

impl fmt::Display for Outer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("outer")
    }
}

impl std::error::Error for Outer {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

#[test]
fn error_chain_prints_every_source() {
    let err = StorageError::backend(Outer(io::Error::other("disk on fire")));
    assert_eq!(
        ErrorChain(&err).to_string(),
        "storage backend failed: outer: disk on fire"
    );
}

#[test]
fn timestamps_are_truncated_to_microseconds() {
    let at = datetime!(2026-01-01 00:00:00.123_456_789 UTC);
    assert_eq!(
        truncate_to_micros(at),
        datetime!(2026-01-01 00:00:00.123_456 UTC)
    );
}
