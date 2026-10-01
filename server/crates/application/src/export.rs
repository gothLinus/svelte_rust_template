use domain::{
    pagination::{MAX_PAGE_SIZE, PageRequest, PageSize},
    repository::{Repository, Resource},
    user::UserId,
};
use serde_json::{Map, Value, json};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::error::AppError;

pub const EXPORTED_TABLES: &[&str] = &[
    "users",
    "user_roles",
    "sessions",
    "external_identities",
    "passkeys",
    "totp_credentials",
    "recovery_codes",
    "audit_events",
    "notes",
];

pub const NOT_EXPORTED_TABLES: &[(&str, &str)] = &[
    (
        "user_tokens",
        "pending emailed links, stored as digests, gone within days",
    ),
    (
        "one_time_codes",
        "pending sign-in codes, stored as digests, gone within minutes",
    ),
    (
        "mfa_challenges",
        "half-finished sign-ins, gone within minutes",
    ),
    (
        "webauthn_challenges",
        "passkey ceremonies in progress, gone within minutes",
    ),
    (
        "oauth_flows",
        "social sign-ins in progress, gone within minutes",
    ),
    (
        "outbox",
        "mail waiting for delivery, sealed, gone once delivered",
    ),
];

pub trait Exportable: Resource {
    const SECTION: &'static str;
    const TABLE: &'static str;

    fn owned_by(user: UserId) -> Self::Filter;

    fn export(&self) -> Value;
}

async fn owned<E: Exportable>(
    store: &mut impl Repository<E>,
    user: UserId,
) -> Result<Value, AppError> {
    let filter = E::owned_by(user);
    let size =
        PageSize::parse(Some(MAX_PAGE_SIZE)).map_err(|err| AppError::invalid("limit", &err))?;
    let mut rows = Vec::new();
    let mut after = None;
    loop {
        let page = store
            .list(&filter, PageRequest::new(size, after.take()))
            .await?;
        rows.extend(page.items.iter().map(Exportable::export));
        match page.next {
            Some(next) => after = Some(next),
            None => return Ok(Value::Array(rows)),
        }
    }
}

#[derive(Debug, Default)]
pub struct Export {
    sections: Map<String, Value>,
}

impl Export {
    pub(crate) fn new(exported_at: OffsetDateTime) -> Self {
        let mut export = Self::default();
        export.add("exported_at", json!(timestamp(exported_at)));
        export
    }

    pub(crate) fn add(&mut self, section: &str, data: Value) {
        self.sections.insert(section.to_owned(), data);
    }

    pub(crate) async fn add_owned<E: Exportable>(
        &mut self,
        store: &mut impl Repository<E>,
        user: UserId,
    ) -> Result<(), AppError> {
        let rows = owned::<E>(store, user).await?;
        self.add(E::SECTION, rows);
        Ok(())
    }

    pub fn into_json(self) -> Value {
        Value::Object(self.sections)
    }
}

pub(crate) fn timestamp(at: OffsetDateTime) -> Option<String> {
    at.format(&Rfc3339).ok()
}
