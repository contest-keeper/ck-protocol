use serde::{Deserialize, Deserializer, Serialize};

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

/// CKCONFIG format for a stick that claims its own key on first boot.
pub const ENROLMENT_SCHEMA: u32 = 3;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Token")]
pub struct EnrolmentCode(Token);

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("an enrolment code needs five groups of four allowed letters or digits")]
pub struct InvalidEnrolmentCode;

impl EnrolmentCode {
    /// # Errors
    ///
    /// Returns an error if the code is outside the Hub's canonical format.
    pub fn parse(text: &str) -> Result<Self, InvalidEnrolmentCode> {
        Self::try_from(Token::new(text))
    }

    #[must_use]
    pub fn expose(&self) -> &str {
        self.0.expose()
    }
}

impl TryFrom<Token> for EnrolmentCode {
    type Error = InvalidEnrolmentCode;

    fn try_from(token: Token) -> Result<Self, Self::Error> {
        const ALPHABET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
        let mut groups = token.expose().split('-');
        if groups.clone().count() == 5
            && groups.all(|group| group.len() == 4 && group.bytes().all(|b| ALPHABET.contains(&b)))
        {
            Ok(Self(token))
        } else {
            Err(InvalidEnrolmentCode)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnrolmentConfig {
    #[serde(deserialize_with = "read_enrolment_schema")]
    schema: u32,
    pub stick_id: StickId,
    pub created_at: LocalDateTime,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine_number: Option<u32>,
    pub enrolment_code: EnrolmentCode,
    #[serde(
        rename = "token",
        default,
        skip_serializing,
        deserialize_with = "reject_legacy_token"
    )]
    _legacy_token: (),
    #[serde(flatten)]
    pub settings: Settings,
}

impl EnrolmentConfig {
    #[must_use]
    pub fn new(
        stick_id: StickId,
        created_at: LocalDateTime,
        machine_number: Option<u32>,
        enrolment_code: EnrolmentCode,
        settings: Settings,
    ) -> Self {
        Self {
            schema: ENROLMENT_SCHEMA,
            stick_id,
            created_at,
            machine_number,
            enrolment_code,
            _legacy_token: (),
            settings,
        }
    }
}

fn read_enrolment_schema<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u32, D::Error> {
    let schema = u32::deserialize(deserializer)?;
    if schema == ENROLMENT_SCHEMA {
        Ok(schema)
    } else {
        Err(serde::de::Error::custom("expected the enrolment schema"))
    }
}

fn reject_legacy_token<'de, D: Deserializer<'de>>(_: D) -> Result<(), D::Error> {
    Err(serde::de::Error::custom(
        "an enrolment config cannot carry a legacy stick token",
    ))
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

    #[test]
    fn enrolment_config_writes_a_code_instead_of_a_long_lived_key() {
        let legacy: StickConfig = serde_json::from_str(SAMPLE).expect("legacy config");
        let code = EnrolmentCode::parse("ABCD-EFGH-JKLM-NPQR-STUV").expect("valid code");
        let config = EnrolmentConfig::new(
            legacy.stick_id,
            legacy.created_at,
            Some(7),
            code,
            legacy.settings,
        );
        let json = serde_json::to_value(&config).expect("writes the config");
        assert_eq!(json["schema"], ENROLMENT_SCHEMA);
        assert_eq!(json["enrolment_code"], "ABCD-EFGH-JKLM-NPQR-STUV");
        assert!(json.get("token").is_none());
        assert!(serde_json::from_value::<StickConfig>(json.clone()).is_err());
        assert_eq!(
            serde_json::from_value::<EnrolmentConfig>(json.clone()).expect("reads the config"),
            config
        );
        assert!(!format!("{config:?}").contains("ABCD-EFGH"));

        let mut wrong_schema = json.clone();
        wrong_schema["schema"] = serde_json::json!(SCHEMA);
        assert!(serde_json::from_value::<EnrolmentConfig>(wrong_schema).is_err());

        let mut mixed = json;
        mixed["token"] = serde_json::Value::Null;
        assert!(serde_json::from_value::<EnrolmentConfig>(mixed).is_err());
    }

    #[test]
    fn an_enrolment_code_has_the_hubs_canonical_format() {
        for bad in [
            "",
            "ABCD-EFGH-IJKL-MNOP-QRST",
            "abcd-EFGH-JKLM-NPQR-STUV",
            "ABCDEFGHJKLMNPQRSTUV",
        ] {
            assert!(EnrolmentCode::parse(bad).is_err());
        }
    }
}
