use serde::{Deserialize, Serialize};

use crate::time::LocalDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RecordingMode {
    Off,
    Snapshots,
    #[default]
    Low,
    Standard,
}

impl RecordingMode {
    pub const ALL: [Self; 4] = [Self::Off, Self::Snapshots, Self::Low, Self::Standard];

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

/// When recordings leave the stick for the Hub.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum UploadMode {
    /// Recordings stay on the stick.
    #[default]
    Keep,
    /// Upload once the contest has ended.
    AfterContest,
    /// Upload while the contest runs, at idle priority.
    DuringContest,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Recording {
    pub mode: RecordingMode,
    /// Seconds to wait after the desktop appears, so boot and login stay fast.
    pub start_delay_s: u64,
    /// Do not start before this time, by the machine's clock.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_at: Option<LocalDateTime>,
    /// The recorder uses 10 seconds when this is left out.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub snapshot_interval_s: Option<u64>,
    pub upload: UploadMode,
    /// Delete recordings from the stick this many days after they were made. Left out means never.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retention_days: Option<u32>,
}

impl Default for Recording {
    fn default() -> Self {
        Self {
            mode: RecordingMode::Low,
            start_delay_s: 60,
            start_at: None,
            snapshot_interval_s: None,
            upload: UploadMode::Keep,
            retention_days: None,
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
    fn upload_defaults_to_keeping_recordings_and_reads_the_other_modes() {
        let old: Recording = serde_json::from_str(r#"{"mode":"low"}"#).expect("valid");
        assert_eq!(old.upload, UploadMode::Keep);
        assert_eq!(old.retention_days, None);
        let json = serde_json::to_value(&old).expect("serializes");
        assert_eq!(json["upload"], "keep");
        assert!(json.get("retention_days").is_none());

        for (text, mode) in [
            ("after_contest", UploadMode::AfterContest),
            ("during_contest", UploadMode::DuringContest),
        ] {
            let json = format!(r#"{{"upload":"{text}","retention_days":14}}"#);
            let recording: Recording = serde_json::from_str(&json).expect("valid");
            assert_eq!(recording.upload, mode);
            assert_eq!(recording.retention_days, Some(14));
        }
    }
}
