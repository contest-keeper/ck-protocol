//! Screen recording options.

use serde::{Deserialize, Serialize};

use crate::time::LocalDateTime;

/// How much the stick records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RecordingMode {
    /// No recording.
    Off,
    /// One picture every few seconds. The cheapest option.
    Snapshots,
    /// 1 frame per second at half resolution.
    #[default]
    Low,
    /// 2 frames per second at full resolution.
    Standard,
}

impl RecordingMode {
    /// Every mode, from cheapest to most expensive.
    pub const ALL: [Self; 4] = [Self::Off, Self::Snapshots, Self::Low, Self::Standard];

    /// The mode's name as written in the settings and in `/etc/ck/recording.conf`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Snapshots => "snapshots",
            Self::Low => "low",
            Self::Standard => "standard",
        }
    }
}

/// When and how the stick records.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Recording {
    /// How much to record.
    pub mode: RecordingMode,
    /// Seconds to wait after the desktop appears, so the boot and login stay fast.
    pub start_delay_s: u64,
    /// Do not start before this time, by the machine's clock.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_at: Option<LocalDateTime>,
    /// Seconds between pictures in `snapshots` mode. The recorder uses 10 when this is left out.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot_interval_s: Option<u64>,
}

impl Default for Recording {
    fn default() -> Self {
        Self {
            mode: RecordingMode::Low,
            start_delay_s: 60,
            start_at: None,
            snapshot_interval_s: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_names_match_the_recorder() {
        for mode in RecordingMode::ALL {
            let json = serde_json::to_string(&mode).expect("serializes");
            assert_eq!(json, format!("\"{}\"", mode.as_str()));
        }
    }

    #[test]
    fn defaults_are_low_after_a_minute() {
        let recording = Recording::default();
        assert_eq!(recording.mode, RecordingMode::Low);
        assert_eq!(recording.start_delay_s, 60);
        assert!(recording.start_at.is_none());
    }

    #[test]
    fn reads_the_start_time() {
        let recording: Recording =
            serde_json::from_str(r#"{"mode":"standard","start_at":"2026-11-15T09:00"}"#)
                .expect("valid json");
        assert_eq!(recording.mode, RecordingMode::Standard);
        assert_eq!(
            recording.start_at.expect("start time").to_string(),
            "2026-11-15T09:00"
        );
        assert_eq!(recording.start_delay_s, 60);
    }

    #[test]
    fn a_null_start_time_means_none() {
        let recording: Recording =
            serde_json::from_str(r#"{"start_at":null}"#).expect("valid json");
        assert!(recording.start_at.is_none());
    }
}
