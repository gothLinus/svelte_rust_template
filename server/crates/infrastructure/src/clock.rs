use domain::clock::{Clock, truncate_to_micros};
use time::OffsetDateTime;

#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> OffsetDateTime {
        truncate_to_micros(OffsetDateTime::now_utc())
    }
}
