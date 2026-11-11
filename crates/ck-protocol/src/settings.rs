//! The settings an admin chooses, and the file written to one stick.

use serde::{Deserialize, Serialize};

use crate::api::Api;
use crate::branding::Branding;
use crate::contest::Contest;
use crate::ids::{StickId, Token};
use crate::network::Network;
use crate::recording::Recording;
use crate::time::LocalDateTime;

/// The version of the settings file written by this crate.
pub const SCHEMA: u32 = 1;

/// Folder of the settings on the EFI partition.
pub const ESP_CONFIG_DIR: &str = "ck";

/// File name of the settings inside [`ESP_CONFIG_DIR`].
pub const ESP_CONFIG_FILE: &str = "config.json";

/// Everything an admin chooses for a contest. The same settings go on every stick of the contest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// The contest.
    pub contest: Contest,
    /// Name of the team these sticks belong to. Used for computer names and in reports.
    pub team: String,
    /// Name of the venue or lab.
    pub site: String,
    /// What contestants see on the machine.
    pub branding: Branding,
    /// Which sites may be opened. Written at the top level of the file.
    #[serde(flatten)]
    pub network: Network,
    /// Screen recording.
    pub recording: Recording,
    /// Where the stick reports to.
    pub api: Api,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            contest: Contest::default(),
            team: "default team".to_owned(),
            site: "default site".to_owned(),
            branding: Branding::default(),
            network: Network::default(),
            recording: Recording::default(),
            api: Api::default(),
        }
    }
}

/// The file `ck/config.json`: the admin's settings plus the identity of one stick.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StickConfig {
    /// The version of this file's format. See [`SCHEMA`].
    pub schema: u32,
    /// Which stick this is.
    pub stick_id: StickId,
    /// When the admin tool made the file, by the admin's clock.
    pub created_at: LocalDateTime,
    /// The stick's secret. Only the admin tool's registry and the stick itself hold it.
    pub token: Token,
    /// The settings, written at the top level of the file.
    #[serde(flatten)]
    pub settings: Settings,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recording::RecordingMode;

    /// A file in the layout of the Python prototype, with the stick's secret moved to the top level.
    const SAMPLE: &str = r#"{
        "schema": 1,
        "stick_id": "03c9f556",
        "created_at": "2026-11-10T15:22:05",
        "token": "abc",
        "contest": {"name": "RCPC 2026 test", "platform": "kilonova", "contest_id": ""},
        "team": "Test Team",
        "site": "Lab 1",
        "allowed_domains": [".cppreference.com", ".rcpc.kilonova.ro"],
        "recording": {"mode": "low", "start_delay_s": 60, "start_at": null},
        "api": {"url": "http://10.0.2.2:8088", "heartbeat_s": 15}
    }"#;

    #[test]
    fn reads_a_stick_file() {
        let config: StickConfig = serde_json::from_str(SAMPLE).expect("valid file");
        assert_eq!(config.schema, SCHEMA);
        assert_eq!(config.stick_id.as_str(), "03c9f556");
        assert_eq!(config.token.expose(), "abc");
        assert_eq!(config.settings.team, "Test Team");
        assert_eq!(config.settings.network.allowed_domains.len(), 2);
        assert_eq!(config.settings.recording.mode, RecordingMode::Low);
        assert_eq!(config.settings.api.url, "http://10.0.2.2:8088");
        assert_eq!(config.settings.branding, Branding::default());
    }

    #[test]
    fn a_stick_file_survives_a_round_trip() {
        let config: StickConfig = serde_json::from_str(SAMPLE).expect("valid file");
        let json = serde_json::to_string(&config).expect("serializes");
        let again: StickConfig = serde_json::from_str(&json).expect("valid file");
        assert_eq!(config, again);
        assert!(!json.contains("null"));
    }

    #[test]
    fn writes_the_sites_at_the_top_level() {
        let json = serde_json::to_value(StickConfig {
            schema: SCHEMA,
            stick_id: StickId::parse("s1").expect("valid id"),
            created_at: "2026-11-10T15:22".parse().expect("valid time"),
            token: Token::new("t"),
            settings: Settings::default(),
        })
        .expect("serializes");
        assert!(json.get("allowed_domains").is_some());
        assert!(json.get("network").is_none());
        assert!(json.get("settings").is_none());
    }

    #[test]
    fn settings_without_any_field_use_the_defaults() {
        let settings: Settings = serde_json::from_str("{}").expect("valid json");
        assert_eq!(settings, Settings::default());
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let settings: Settings =
            serde_json::from_str(r#"{"team":"T","from_the_future":1}"#).expect("valid json");
        assert_eq!(settings.team, "T");
    }

    #[test]
    fn a_stick_file_needs_its_identity() {
        assert!(serde_json::from_str::<StickConfig>(r#"{"team":"T"}"#).is_err());
        assert!(
            serde_json::from_str::<StickConfig>(&SAMPLE.replace("03c9f556", "bad id")).is_err()
        );
    }
}
