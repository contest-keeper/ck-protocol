//! What contestants see on the machine. The admin chooses all of it.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bookmark {
    pub title: String,
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Branding {
    pub display_name: String,
    /// Placeholders: `{team}`, `{site}` and `{n}`.
    pub computer_name_pattern: String,
    /// At most 11 characters, FAT rules.
    pub usb_label: String,
    /// File name only; the file sits next to `config.json`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wallpaper: Option<String>,
    pub bookmarks: Vec<Bookmark>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub home_page: Option<String>,
    pub welcome_text: String,
}

impl Default for Branding {
    fn default() -> Self {
        Self {
            display_name: String::new(),
            computer_name_pattern: "{team}-{n}".to_owned(),
            usb_label: "CONTEST".to_owned(),
            wallpaper: None,
            bookmarks: Vec::new(),
            home_page: None,
            welcome_text: String::new(),
        }
    }
}

pub const COMPUTER_NAME_PLACEHOLDERS: [&str; 3] = ["{team}", "{site}", "{n}"];

pub const MAX_COMPUTER_NAME_LENGTH: usize = 63;

/// Fills the placeholders and makes the result a valid host name: lowercase letters and digits,
/// each run of anything else becomes one `-`, no `-` at the ends, at most 63 characters.
#[must_use]
pub fn render_computer_name(pattern: &str, team: &str, site: &str, number: u32) -> String {
    let filled = pattern
        .replace("{team}", team)
        .replace("{site}", site)
        .replace("{n}", &number.to_string());

    let mut name = String::new();
    for character in filled.chars() {
        if character.is_ascii_alphanumeric() {
            name.push(character.to_ascii_lowercase());
        } else if !name.ends_with('-') {
            name.push('-');
        }
    }
    let trimmed = name.trim_matches('-');
    trimmed
        .chars()
        .take(MAX_COMPUTER_NAME_LENGTH)
        .collect::<String>()
        .trim_end_matches('-')
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fills_placeholders() {
        assert_eq!(
            render_computer_name("{team}-{n}", "Team A", "Lab 1", 7),
            "team-a-7"
        );
        assert_eq!(
            render_computer_name("{site}-{team}-{n}", "T", "Lab 1", 12),
            "lab-1-t-12"
        );
    }

    #[test]
    fn makes_names_safe_for_a_host_name() {
        assert_eq!(
            render_computer_name("{team}", "  Ünï  cødé!! ", "", 1),
            "n-c-d"
        );
        assert_eq!(render_computer_name("{team}", "---", "", 1), "");
    }

    #[test]
    fn cuts_long_names_without_a_trailing_dash() {
        let name = render_computer_name("{team}", &"a-".repeat(60), "", 1);
        assert!(name.len() <= MAX_COMPUTER_NAME_LENGTH);
        assert!(!name.ends_with('-'));
    }

    #[test]
    fn defaults_are_usable() {
        let branding = Branding::default();
        assert_eq!(branding.computer_name_pattern, "{team}-{n}");
        assert!(branding.usb_label.len() <= 11);
    }

    #[test]
    fn missing_optional_parts_are_left_out_when_written() {
        let json = serde_json::to_string(&Branding::default()).expect("serializes");
        assert!(
            !json.contains("wallpaper") && !json.contains("home_page") && !json.contains("null")
        );
    }
}
