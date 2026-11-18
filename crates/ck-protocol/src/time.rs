use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use time::format_description::well_known::Rfc3339;
use time::format_description::{BorrowedFormatItem, parse_borrowed};
use time::{OffsetDateTime, PrimitiveDateTime, UtcOffset};

/// A date and time as shown on the venue's wall clock, so it has no time zone.
/// Reads `YYYY-MM-DDTHH:MM[:SS]` (a space may replace the `T`); writes seconds only when they are not zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocalDateTime(PrimitiveDateTime);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("'{0}' is not a date and time like 2026-11-15T09:00")]
pub struct ParseLocalDateTimeError(String);

const READ_FORMATS: [&str; 4] = [
    "[year]-[month]-[day]T[hour]:[minute]",
    "[year]-[month]-[day]T[hour]:[minute]:[second]",
    "[year]-[month]-[day] [hour]:[minute]",
    "[year]-[month]-[day] [hour]:[minute]:[second]",
];
const WRITE_MINUTES: &str = "[year]-[month]-[day]T[hour]:[minute]";
const WRITE_SECONDS: &str = "[year]-[month]-[day]T[hour]:[minute]:[second]";

fn parse_with(text: &str, format: &str) -> Option<PrimitiveDateTime> {
    let items: Vec<BorrowedFormatItem<'_>> = parse_borrowed::<2>(format).ok()?;
    PrimitiveDateTime::parse(text, &items).ok()
}

impl LocalDateTime {
    #[must_use]
    pub fn as_primitive(self) -> PrimitiveDateTime {
        self.0
    }
}

impl From<PrimitiveDateTime> for LocalDateTime {
    fn from(value: PrimitiveDateTime) -> Self {
        Self(value)
    }
}

impl FromStr for LocalDateTime {
    type Err = ParseLocalDateTimeError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let trimmed = text.trim();
        READ_FORMATS
            .iter()
            .find_map(|format| parse_with(trimmed, format))
            .map(Self)
            .ok_or_else(|| ParseLocalDateTimeError(text.to_owned()))
    }
}

impl fmt::Display for LocalDateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let format = if self.0.second() == 0 {
            WRITE_MINUTES
        } else {
            WRITE_SECONDS
        };
        let items = parse_borrowed::<2>(format).map_err(|_| fmt::Error)?;
        let text = self.0.format(&items).map_err(|_| fmt::Error)?;
        f.write_str(&text)
    }
}

impl Serialize for LocalDateTime {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for LocalDateTime {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

/// A moment in time, always in UTC. Reads any RFC 3339 time; writes `2026-11-10T09:30:00Z` with
/// 0, 3, 6 or 9 fraction digits, as few as keep the value exact. This is the text the `chrono`
/// version wrote, which matters because events are hashed and signed over their JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timestamp(OffsetDateTime);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("'{0}' is not an RFC 3339 time like 2026-11-10T09:30:00Z")]
pub struct ParseTimestampError(String);

impl Timestamp {
    #[must_use]
    pub fn as_offset_date_time(self) -> OffsetDateTime {
        self.0
    }
}

impl From<OffsetDateTime> for Timestamp {
    fn from(value: OffsetDateTime) -> Self {
        Self(value.to_offset(UtcOffset::UTC))
    }
}

impl FromStr for Timestamp {
    type Err = ParseTimestampError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        OffsetDateTime::parse(text, &Rfc3339)
            .map(Self::from)
            .map_err(|_| ParseTimestampError(text.to_owned()))
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let t = self.0;
        write!(
            f,
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
            t.year(),
            u8::from(t.month()),
            t.day(),
            t.hour(),
            t.minute(),
            t.second()
        )?;
        let nanos = t.nanosecond();
        if nanos == 0 {
            f.write_str("Z")
        } else if nanos.is_multiple_of(1_000_000) {
            write!(f, ".{:03}Z", nanos / 1_000_000)
        } else if nanos.is_multiple_of(1_000) {
            write!(f, ".{:06}Z", nanos / 1_000)
        } else {
            write!(f, ".{nanos:09}Z")
        }
    }
}

impl Serialize for Timestamp {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_accepted_forms_and_rejects_the_rest() {
        for text in [
            "2026-11-15T09:00",
            "2026-11-15T09:00:00",
            "2026-11-15 09:00",
            " 2026-11-15T09:00 ",
        ] {
            let parsed: LocalDateTime = text.parse().expect(text);
            assert_eq!(parsed.to_string(), "2026-11-15T09:00");
        }
        for text in [
            "",
            "tomorrow",
            "2026-11-15",
            "2026-13-01T09:00",
            "2026-11-15T25:00",
            "09:00",
        ] {
            assert!(
                text.parse::<LocalDateTime>().is_err(),
                "{text:?} should be rejected"
            );
        }
    }

    #[test]
    fn local_times_keep_their_seconds_only_when_not_zero() {
        let parsed: LocalDateTime = "2026-11-15 09:00:07".parse().expect("valid");
        assert_eq!(parsed.to_string(), "2026-11-15T09:00:07");
    }

    /// What `chrono` wrote for the same instants (its `AutoSi` format), kept as literals.
    #[test]
    fn timestamps_are_written_exactly_like_chrono_did() {
        for (input, written) in [
            ("2026-11-10T09:30:00Z", "2026-11-10T09:30:00Z"),
            ("2026-11-10T09:30:00.500Z", "2026-11-10T09:30:00.500Z"),
            ("2026-11-10T09:30:00.5Z", "2026-11-10T09:30:00.500Z"),
            ("2026-11-10T09:30:00.123456Z", "2026-11-10T09:30:00.123456Z"),
            (
                "2026-11-10T09:30:00.123456789Z",
                "2026-11-10T09:30:00.123456789Z",
            ),
            ("2026-11-10T09:30:00.000Z", "2026-11-10T09:30:00Z"),
            ("2026-11-10T11:30:00+02:00", "2026-11-10T09:30:00Z"),
            ("2026-11-10T09:30:00z", "2026-11-10T09:30:00Z"),
        ] {
            let parsed: Timestamp = input.parse().expect(input);
            assert_eq!(parsed.to_string(), written, "from {input}");
            let json = serde_json::to_string(&parsed).expect("serializes");
            assert_eq!(json, format!("\"{written}\""));
        }
    }

    #[test]
    fn bad_timestamps_are_rejected() {
        for text in ["", "yesterday", "2026-11-10T09:30:00", "2026-11-10"] {
            assert!(text.parse::<Timestamp>().is_err(), "{text:?}");
        }
    }
}
