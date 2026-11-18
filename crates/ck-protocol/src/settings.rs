use serde::{Deserialize, Serialize};

use crate::api::Api;
use crate::branding::{Branding, render_computer_name};
use crate::contest::Contest;
use crate::ids::{StickId, Token};
use crate::network::Network;
use crate::recording::Recording;
use crate::time::LocalDateTime;
use crate::wifi::Wifi;

/// The format version written today: the file lives on the `CKCONFIG` partition.
pub const SCHEMA: u32 = 2;

/// The first format, kept on the EFI partition as `ck/config.json`. Still read for one release so
/// sticks made before `CKCONFIG` keep working; the fields are the same.
pub const LEGACY_SCHEMA: u32 = 1;

/// Volume label of the small FAT partition that holds one stick's identity and settings.
pub const CONFIG_PARTITION_LABEL: &str = "CKCONFIG";

/// Where the agent mounts it, read-only.
pub const CONFIG_MOUNT_POINT: &str = "/ck-config";

/// The config file, at the root of the partition. `branding.wallpaper` names a file next to it.
pub const CONFIG_FILE: &str = "config.json";

/// Where schema 1 kept the file, on the EFI partition.
pub const ESP_CONFIG_DIR: &str = "ck";

pub const ESP_CONFIG_FILE: &str = "config.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub contest: Contest,
    pub team: String,
    pub site: String,
    pub branding: Branding,
    /// Written at the top level of the file.
    #[serde(flatten)]
    pub network: Network,
    pub recording: Recording,
    pub api: Api,
    /// The venue network. The secret inside is written only to the stick's config file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wifi: Option<Wifi>,
    /// The agent switches Bluetooth off with `rfkill` at every boot. Off by default, so sticks made
    /// before this setting existed behave as before.
    pub block_bluetooth: bool,
    /// Password hash for the `icpcadmin` account, in the format `chpasswd -e` takes (for example
    /// `$6$salt$hash`). Only the hash is stored; the agent applies it at boot. Left out means the
    /// account keeps the image default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admin_password_hash: Option<String>,
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
            wifi: None,
            block_bluetooth: false,
            admin_password_hash: None,
        }
    }
}

/// `config.json` on the `CKCONFIG` partition: the identity of one stick (id, token) and its settings
/// (Hub address, contest, wallpaper name, Wi-Fi). Tell versions apart by `schema`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StickConfig {
    pub schema: u32,
    pub stick_id: StickId,
    pub created_at: LocalDateTime,
    /// Held only by the admin tool's registry and the stick itself.
    pub token: Token,
    /// Which machine of the contest this is, for `{n}` in the computer name. The admin tool counts
    /// them. Sticks made before numbering existed have none and count as 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine_number: Option<u32>,
    #[serde(flatten)]
    pub settings: Settings,
}

impl StickConfig {
    #[must_use]
    pub fn computer_name(&self) -> String {
        render_computer_name(
            &self.settings.branding.computer_name_pattern,
            &self.settings.team,
            &self.settings.site,
            self.machine_number.unwrap_or(1),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file in the Python prototype's layout, with the secret moved to the top level.
    const SAMPLE: &str = r#"{
        "schema": 1,
        "stick_id": "03c9f556",
        "created_at": "2026-11-10T15:22:05",
        "token": "abc",
        "contest": {"name": "Example contest", "platform": "kilonova", "contest_id": ""},
        "team": "Test Team",
        "site": "Lab 1",
        "allowed_domains": [".cppreference.com", ".judge.example.org"],
        "recording": {"mode": "low", "start_delay_s": 60, "start_at": null},
        "api": {"url": "http://10.0.2.2:8088", "heartbeat_s": 15},
        "added_by_a_newer_version": true
    }"#;

    #[test]
    fn reads_and_rewrites_a_stick_file() {
        let config: StickConfig = serde_json::from_str(SAMPLE).expect("valid file");
        assert_eq!(config.stick_id.as_str(), "03c9f556");
        assert_eq!(config.settings.network.allowed_domains.len(), 2);
        assert_eq!(config.settings.api.url, "http://10.0.2.2:8088");

        let json = serde_json::to_value(&config).expect("serializes");
        assert!(json.get("allowed_domains").is_some() && json.get("network").is_none());
        assert!(!json.to_string().contains("null"));
        assert_eq!(
            serde_json::from_value::<StickConfig>(json).expect("valid file"),
            config
        );
    }

    #[test]
    fn bluetooth_is_not_blocked_unless_asked() {
        let config: StickConfig = serde_json::from_str(SAMPLE).expect("valid file");
        assert!(
            !config.settings.block_bluetooth,
            "an older file has no setting"
        );

        let blocked = SAMPLE.replace("\"team\"", "\"block_bluetooth\": true, \"team\"");
        let config: StickConfig = serde_json::from_str(&blocked).expect("valid file");
        assert!(config.settings.block_bluetooth);
        let json = serde_json::to_value(&config).expect("serializes");
        assert_eq!(json["block_bluetooth"], true);
    }

    #[test]
    fn the_machine_number_is_optional_and_names_the_computer() {
        let mut config: StickConfig = serde_json::from_str(SAMPLE).expect("valid file");
        assert_eq!(config.machine_number, None, "an older file has no number");
        assert!(
            !serde_json::to_string(&config)
                .expect("json")
                .contains("machine_number")
        );
        assert_eq!(config.computer_name(), "test-team-1");

        config.machine_number = Some(7);
        assert_eq!(config.computer_name(), "test-team-7");
        let json = serde_json::to_string(&config).expect("json");
        assert_eq!(
            serde_json::from_str::<StickConfig>(&json).expect("valid file"),
            config
        );
    }

    #[test]
    fn a_stick_file_needs_a_valid_identity() {
        assert!(serde_json::from_str::<StickConfig>(r#"{"team":"T"}"#).is_err());
        assert!(
            serde_json::from_str::<StickConfig>(&SAMPLE.replace("03c9f556", "bad id")).is_err()
        );
    }
}
