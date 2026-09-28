use std::{str::FromStr, time::Duration};

use super::ConfigProblem;

/// Reads variables, recording every problem and substituting defaults so reading can go on. Empty
/// values count as unset.
pub(super) struct Reader<'a> {
    pub(super) lookup: &'a dyn Fn(&str) -> Option<String>,
    pub(super) problems: Vec<ConfigProblem>,
    /// `APP_URL` points at the developer's own machine. Development-only settings (mail and texts
    /// written to the log, the published development key) are refused anywhere else.
    pub(super) local: bool,
    pub(super) warnings: Vec<String>,
}

impl Reader<'_> {
    pub(super) fn optional<T>(
        &mut self,
        name: &'static str,
        parse: impl FnOnce(&str) -> Result<T, String>,
    ) -> Option<T> {
        let raw = self.raw(name)?;
        parse(&raw)
            .map_err(|message| self.problem(name, &message))
            .ok()
    }

    pub(super) fn raw(&self, name: &str) -> Option<String> {
        (self.lookup)(name)
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
    }

    pub(super) fn problem(&mut self, variable: &'static str, message: &str) {
        self.problems.push(ConfigProblem {
            variable,
            message: message.to_owned(),
        });
    }

    pub(super) fn parse<T>(
        &mut self,
        name: &'static str,
        default: T,
        parse: impl FnOnce(&str) -> Result<T, String>,
    ) -> T {
        let Some(raw) = self.raw(name) else {
            return default;
        };
        parse(&raw).unwrap_or_else(|message| {
            self.problem(name, &message);
            default
        })
    }

    pub(super) fn required<T>(
        &mut self,
        name: &'static str,
        parse: impl FnOnce(&str) -> Result<T, String>,
    ) -> Option<T> {
        let Some(raw) = self.raw(name) else {
            self.problem(name, "is required");
            return None;
        };
        parse(&raw)
            .map_err(|message| self.problem(name, &message))
            .ok()
    }

    pub(super) fn bool(&mut self, name: &'static str, default: bool) -> bool {
        self.parse(name, default, |raw| match raw {
            "true" | "1" => Ok(true),
            "false" | "0" => Ok(false),
            _ => Err("must be `true` or `false`".to_owned()),
        })
    }

    pub(super) fn number<T: FromStr>(&mut self, name: &'static str, default: T) -> T {
        self.parse(name, default, |raw| {
            raw.parse()
                .map_err(|_| "must be a non-negative whole number".to_owned())
        })
    }

    /// A duration of at most `max`. Every duration is bounded: an absurd value (a session lifetime
    /// of ten million days) would otherwise overflow the date arithmetic on the first sign-in
    /// instead of failing here.
    pub(super) fn duration(
        &mut self,
        name: &'static str,
        default: Duration,
        max: Duration,
    ) -> Duration {
        let value = self.parse(name, default, parse_duration);
        if value > max {
            self.problem(name, &format!("must be at most {}", describe(max)));
            return default;
        }
        value
    }
}

pub fn parse_duration(raw: &str) -> Result<Duration, String> {
    const ERROR: &str = "must be a duration such as `30s`, `15m`, `12h` or `7d`";

    let (digits, unit) = raw
        .find(|c: char| !c.is_ascii_digit())
        .map_or((raw, ""), |split| raw.split_at(split));
    let value: u64 = digits.parse().map_err(|_| ERROR.to_owned())?;
    let seconds_per_unit = match unit.trim() {
        "" | "s" => 1,
        "m" => 60,
        "h" => 60 * 60,
        "d" => 24 * 60 * 60,
        _ => return Err(ERROR.to_owned()),
    };
    value
        .checked_mul(seconds_per_unit)
        .map(Duration::from_secs)
        .ok_or_else(|| ERROR.to_owned())
}

/// `7d`, `12h`, `15m` or `30s`: the largest unit that divides the duration.
fn describe(duration: Duration) -> String {
    let secs = duration.as_secs();
    for (unit, size) in [("d", 86_400), ("h", 3_600), ("m", 60)] {
        if secs >= size && secs.is_multiple_of(size) {
            return format!("{}{unit}", secs / size);
        }
    }
    format!("{secs}s")
}

pub(super) const DAY: Duration = Duration::from_hours(24);

pub(super) fn unsigned(duration: time::Duration) -> Duration {
    duration.try_into().unwrap_or(Duration::ZERO)
}

pub(super) fn signed(duration: Duration) -> time::Duration {
    duration.try_into().unwrap_or(time::Duration::MAX)
}
