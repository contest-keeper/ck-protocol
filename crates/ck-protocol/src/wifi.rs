use std::fmt;

use serde::{Deserialize, Serialize};

use crate::ids::Token;

/// The Wi-Fi network a stick joins at the venue. The agent turns it into a `NetworkManager`
/// connection that only root can read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Wifi {
    pub ssid: String,
    /// Left out for an open network.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<WifiPassword>,
}

/// The Wi-Fi password. It has no `Display` and its `Debug` hides the value, so it cannot reach a
/// log or an event by accident. It is written only inside the stick's config file.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WifiPassword(Token);

impl WifiPassword {
    #[must_use]
    pub fn new(secret: impl Into<String>) -> Self {
        Self(Token::new(secret))
    }

    /// Only for writing the `NetworkManager` connection.
    #[must_use]
    pub fn expose(&self) -> &str {
        self.0.expose()
    }
}

impl fmt::Debug for WifiPassword {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("WifiPassword(hidden)")
    }
}

/// The SSID is at most 32 bytes and has no control characters.
pub const MAX_SSID_BYTES: usize = 32;

/// WPA2 personal passphrases are 8 to 63 printable ASCII characters.
pub const MIN_PASSWORD_LENGTH: usize = 8;
pub const MAX_PASSWORD_LENGTH: usize = 63;

#[must_use]
pub fn is_valid_ssid(ssid: &str) -> bool {
    (1..=MAX_SSID_BYTES).contains(&ssid.len()) && !ssid.chars().any(char::is_control)
}

#[must_use]
pub fn is_valid_password(password: &str) -> bool {
    (MIN_PASSWORD_LENGTH..=MAX_PASSWORD_LENGTH).contains(&password.len())
        && password
            .chars()
            .all(|c| c.is_ascii() && !c.is_ascii_control())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_password_does_not_show_in_debug_output() {
        let wifi = Wifi {
            ssid: "venue".to_owned(),
            password: Some(WifiPassword::new("super-secret-pass")),
        };
        let shown = format!("{wifi:?}");
        assert!(!shown.contains("super-secret-pass"));
        assert!(shown.contains("venue"));
    }

    #[test]
    fn an_open_network_has_no_password_in_its_file() {
        let wifi: Wifi = serde_json::from_str(r#"{"ssid":"open-net"}"#).expect("valid");
        assert_eq!(wifi.password, None);
        assert_eq!(
            serde_json::to_string(&wifi).expect("json"),
            r#"{"ssid":"open-net"}"#
        );
    }

    #[test]
    fn the_password_round_trips_in_the_file() {
        let json = r#"{"ssid":"venue","password":"correct horse"}"#;
        let wifi: Wifi = serde_json::from_str(json).expect("valid");
        assert_eq!(
            wifi.password.as_ref().map(WifiPassword::expose),
            Some("correct horse")
        );
        assert_eq!(serde_json::to_string(&wifi).expect("json"), json);
    }

    #[test]
    fn checks_ssids_and_passwords() {
        assert!(is_valid_ssid("Contest Wi-Fi"));
        assert!(!is_valid_ssid(""));
        assert!(!is_valid_ssid(&"x".repeat(33)));
        assert!(!is_valid_ssid("a\nb"));
        assert!(is_valid_password("12345678"));
        assert!(!is_valid_password("short"));
        assert!(!is_valid_password(&"x".repeat(64)));
        assert!(!is_valid_password("pässword1"));
        assert!(!is_valid_password("tab\tpassword"));
    }
}
