//! The message a stick sends every few seconds to say it is alive.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::StickId;
use crate::recording::RecordingMode;

/// What the recorder is doing right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecorderState {
    /// Recording is off in the settings.
    Off,
    /// Waiting for the desktop, the start delay or the start time.
    Waiting,
    /// Recording.
    Recording,
    /// Not recording because the disk is nearly full.
    LowDisk,
}

/// The recorder as the stick reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecorderStatus {
    /// The mode in the settings.
    pub mode: RecordingMode,
    /// What it is doing.
    pub state: RecorderState,
}

/// Sent by the agent every few seconds. Login and submission events are separate messages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Heartbeat {
    /// Which stick sent it.
    pub stick_id: StickId,
    /// Identifies this boot. It changes every time the stick starts.
    pub boot_id: String,
    /// Counts messages within one boot, starting at 1. A gap means a message was lost.
    pub seq: u64,
    /// When the stick sent it, by the stick's clock. The Hub records its own receive time too.
    pub sent_at: DateTime<Utc>,
    /// Seconds since the stick booted.
    pub uptime_s: u64,
    /// The computer name.
    pub hostname: String,
    /// Content of `/etc/ck/image-version`.
    pub image_version: String,
    /// SHA-256 of the settings file the stick applied, as lowercase hex.
    pub config_sha256: String,
    /// Team from the settings.
    pub team: String,
    /// Site from the settings.
    pub site: String,
    /// The recorder.
    pub recording: RecorderStatus,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Heartbeat {
        Heartbeat {
            stick_id: StickId::parse("03c9f556").expect("valid id"),
            boot_id: "7f1d2c3e-0b4a-4c55-9a64-1d2e3f405162".to_owned(),
            seq: 3,
            sent_at: "2026-11-10T09:30:00Z".parse().expect("valid time"),
            uptime_s: 45,
            hostname: "team-a-7".to_owned(),
            image_version: "ck-os-base development".to_owned(),
            config_sha256: "0".repeat(64),
            team: "Team A".to_owned(),
            site: "Lab 1".to_owned(),
            recording: RecorderStatus {
                mode: RecordingMode::Low,
                state: RecorderState::Recording,
            },
        }
    }

    #[test]
    fn survives_a_round_trip() {
        let json = serde_json::to_string(&sample()).expect("serializes");
        let again: Heartbeat = serde_json::from_str(&json).expect("valid json");
        assert_eq!(again, sample());
    }

    #[test]
    fn writes_the_time_as_rfc_3339_utc() {
        let json = serde_json::to_value(sample()).expect("serializes");
        assert_eq!(json["sent_at"], "2026-11-10T09:30:00Z");
        assert_eq!(json["recording"]["state"], "recording");
    }

    #[test]
    fn state_names_are_snake_case() {
        assert_eq!(
            serde_json::to_string(&RecorderState::LowDisk).expect("serializes"),
            "\"low_disk\""
        );
    }

    #[test]
    fn ignores_fields_it_does_not_know() {
        let mut json = serde_json::to_value(sample()).expect("serializes");
        json["added_later"] = serde_json::json!(true);
        assert!(serde_json::from_value::<Heartbeat>(json).is_ok());
    }

    #[test]
    fn rejects_a_message_without_a_stick() {
        let mut json = serde_json::to_value(sample()).expect("serializes");
        json.as_object_mut().expect("object").remove("stick_id");
        assert!(serde_json::from_value::<Heartbeat>(json).is_err());
    }
}
