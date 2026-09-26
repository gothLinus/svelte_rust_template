use std::{fmt::Write as _, io, path::Path};

pub fn export(dir: impl AsRef<Path>) -> io::Result<()> {
    std::fs::write(dir.as_ref().join("limits.ts"), limits())
}

/// The field limits of the domain's value objects, the number of recovery codes and the default
/// lifetimes of links and codes, as TypeScript constants, so the frontend uses the server's
/// numbers. The SQL `CHECK` constraints repeat them as a backstop; change both together.
pub fn limits() -> String {
    use domain::{mfa, note, passkey, user, user_token::TokenPolicy};

    use crate::{admin::dto as admin, auth::dto::LinkLifetimesDto};

    // What `/auth/methods` says while the server cannot be asked: the defaults.
    let lifetimes = LinkLifetimesDto::from(TokenPolicy::default());
    let count = |value: u32| usize::try_from(value).unwrap_or(usize::MAX);

    let limits: &[(&str, usize)] = &[
        ("MIN_PASSWORD_LENGTH", user::MIN_PASSWORD_LEN),
        ("MAX_PASSWORD_LENGTH", user::MAX_PASSWORD_LEN),
        ("MAX_EMAIL_LENGTH", user::MAX_EMAIL_LEN),
        ("MIN_USERNAME_LENGTH", user::MIN_USERNAME_LEN),
        ("MAX_USERNAME_LENGTH", user::MAX_USERNAME_LEN),
        ("MAX_PASSKEY_NAME_LENGTH", passkey::MAX_PASSKEY_NAME_LEN),
        ("MAX_NOTE_TITLE_LENGTH", note::MAX_NOTE_TITLE_LEN),
        ("MAX_NOTE_BODY_LENGTH", note::MAX_NOTE_BODY_LEN),
        ("MIN_USER_SEARCH_LENGTH", admin::MIN_SEARCH_LEN),
        ("MAX_USER_SEARCH_LENGTH", admin::MAX_SEARCH_LEN),
        ("RECOVERY_CODE_COUNT", mfa::RECOVERY_CODE_COUNT),
        ("DEFAULT_SIGN_IN_MINUTES", count(lifetimes.sign_in_minutes)),
        (
            "DEFAULT_TEXT_CODE_MINUTES",
            count(lifetimes.text_code_minutes),
        ),
        (
            "DEFAULT_PASSWORD_RESET_MINUTES",
            count(lifetimes.password_reset_minutes),
        ),
        (
            "DEFAULT_EMAIL_VERIFICATION_HOURS",
            count(lifetimes.email_verification_hours),
        ),
        (
            "DEFAULT_EMAIL_CHANGE_HOURS",
            count(lifetimes.email_change_hours),
        ),
    ];
    let mut out = String::from(
        "// This file was generated from the Rust domain constants by `just gen-types`. Do not edit it manually.\n",
    );
    for &(name, value) in limits {
        let _infallible = writeln!(out, "export const {name} = {value};");
    }
    out
}
