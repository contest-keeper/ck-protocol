//! The message a stick sends every few seconds to say it is alive.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::StickId;
use crate::recording::RecordingMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecorderState {
    Off,
    Waiting,
    Recording,
    LowDisk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecorderStatus {
    pub mode: RecordingMode,
    pub state: RecorderState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Heartbeat {
    pub stick_id: StickId,
    /// Changes at every boot.
    pub boot_id: String,
    /// Starts at 1 each boot. A gap means a message was lost.
    pub seq: u64,
    /// By the stick's clock. The Hub also records its own receive time.
    pub sent_at: DateTime<Utc>,
    pub uptime_s: u64,
    pub hostname: String,
    pub image_version: String,
    pub config_sha256: String,
    pub team: String,
    pub site: String,
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
