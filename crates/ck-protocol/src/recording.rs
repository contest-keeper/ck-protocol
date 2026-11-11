//! Screen recording options.

use serde::{Deserialize, Serialize};

use crate::time::LocalDateTime;

/// The modes of the `ck-recorder` service.
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
}
