//! Wall-clock times as an admin types them.

use std::fmt;
use std::str::FromStr;

use chrono::NaiveDateTime;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A date and time as shown on the venue's wall clock, so it has no time zone.
/// Reads `YYYY-MM-DDTHH:MM[:SS]` (a space may replace the `T`); writes seconds only when they are not zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocalDateTime(NaiveDateTime);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("'{0}' is not a date and time like 2026-11-15T09:00")]
pub struct ParseLocalDateTimeError(String);

impl LocalDateTime {
    const FORMATS: [&'static str; 4] = [
        "%Y-%m-%dT%H:%M",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M",
        "%Y-%m-%d %H:%M:%S",
    ];

    #[must_use]
    pub fn as_naive(self) -> NaiveDateTime {
        self.0
    }
}

impl From<NaiveDateTime> for LocalDateTime {
    fn from(value: NaiveDateTime) -> Self {
        Self(value)
    }
}

impl FromStr for LocalDateTime {
    type Err = ParseLocalDateTimeError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let trimmed = text.trim();
        Self::FORMATS
            .iter()
            .find_map(|format| NaiveDateTime::parse_from_str(trimmed, format).ok())
            .map(Self)
            .ok_or_else(|| ParseLocalDateTimeError(text.to_owned()))
    }
}

impl fmt::Display for LocalDateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use chrono::Timelike;
        let format = if self.0.second() == 0 {
            "%Y-%m-%dT%H:%M"
        } else {
            "%Y-%m-%dT%H:%M:%S"
        };
        write!(f, "{}", self.0.format(format))
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
}
