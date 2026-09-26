use time::OffsetDateTime;

pub trait Clock: Send + Sync + 'static {
    fn now(&self) -> OffsetDateTime;
}

/// Drops sub-microsecond precision, the resolution PostgreSQL stores, so a value compares equal
/// after a round trip through the database.
pub fn truncate_to_micros(at: OffsetDateTime) -> OffsetDateTime {
    at.replace_nanosecond(at.microsecond() * 1_000)
        .unwrap_or(at)
}
