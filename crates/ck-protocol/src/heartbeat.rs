use serde::{Deserialize, Serialize};

use crate::ids::StickId;
use crate::recording::RecordingMode;
use crate::time::Timestamp;

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

/// What a stick made of its settings file at the last boot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConfigTrust {
    /// The file carries the Hub's signature and the stick applied it.
    Verified,
    /// A settings file was refused (edited, unsigned or signed for another stick), and the stick
    /// went on with the last settings it had verified, or with the image's own defaults.
    Rejected,
    /// The image carries no key to check with, so the file was applied as it was.
    Unchecked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Heartbeat {
    pub stick_id: StickId,
    /// Changes at every boot.
    pub boot_id: String,
    /// Starts at 1 each boot. A gap means a message was lost.
    pub seq: u64,
    /// By the stick's clock. The Hub also records its own receive time.
    pub sent_at: Timestamp,
    pub uptime_s: u64,
    pub hostname: String,
    pub image_version: String,
    pub config_sha256: String,
    pub team: String,
    pub site: String,
    pub recording: RecorderStatus,
    /// The Hub can send `unlock_screen` only to a stick that advertises it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub screen_unlock: bool,
    /// Left out by a stick that predates the signature check.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config_trust: Option<ConfigTrust>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn survives_a_round_trip_with_an_rfc_3339_time() {
        let heartbeat = Heartbeat {
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
            screen_unlock: false,
            config_trust: None,
        };
        let json = serde_json::to_value(&heartbeat).expect("serializes");
        assert_eq!(json["sent_at"], "2026-11-10T09:30:00Z");
        assert_eq!(
            serde_json::from_value::<Heartbeat>(json).expect("valid json"),
            heartbeat
        );
    }

    #[test]
    fn the_settings_check_is_reported_only_by_sticks_that_make_it() {
        let old = r#"{"stick_id":"03c9f556","boot_id":"b","seq":3,"sent_at":"2026-11-10T09:30:00Z","uptime_s":45,"hostname":"h","image_version":"v","config_sha256":"0","team":"T","site":"S","recording":{"mode":"low","state":"recording"}}"#;
        let parsed: Heartbeat = serde_json::from_str(old).expect("old heartbeat");
        assert_eq!(parsed.config_trust, None);
        assert!(
            !serde_json::to_string(&parsed)
                .expect("json")
                .contains("config_trust")
        );

        let new = old.replace("}}", r#"},"config_trust":"rejected"}"#);
        let parsed: Heartbeat = serde_json::from_str(&new).expect("new heartbeat");
        assert_eq!(parsed.config_trust, Some(ConfigTrust::Rejected));
        assert!(
            serde_json::to_string(&parsed)
                .expect("json")
                .contains(r#""config_trust":"rejected""#)
        );
    }

    #[test]
    fn a_heartbeat_from_before_the_time_crate_swap_keeps_its_bytes() {
        let old = r#"{"stick_id":"03c9f556","boot_id":"b","seq":3,"sent_at":"2026-11-10T09:30:00.123456789Z","uptime_s":45,"hostname":"h","image_version":"v","config_sha256":"0","team":"T","site":"S","recording":{"mode":"low","state":"recording"}}"#;
        let parsed: Heartbeat = serde_json::from_str(old).expect("old heartbeat");
        assert_eq!(serde_json::to_string(&parsed).expect("json"), old);
    }
}
