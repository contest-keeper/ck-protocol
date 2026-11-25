//! Checks on the settings, with problems described in plain words for the admin.

use crate::api::is_valid_http_url;
use crate::branding::{Branding, COMPUTER_NAME_PLACEHOLDERS};
use crate::network::is_valid_domain;
use crate::settings::{LEGACY_SCHEMA, SCHEMA, Settings, StickConfig};
use crate::wifi::{
    MAX_PASSWORD_LENGTH, MAX_SSID_BYTES, MIN_PASSWORD_LENGTH, is_valid_password, is_valid_ssid,
};

pub const MAX_USB_LABEL_LENGTH: usize = 11;

/// Longest team or site name. Both end up in one-line files and in heartbeats.
pub const MAX_IDENTITY_LENGTH: usize = 64;

// FAT forbids these in a volume label, besides control characters.
const FORBIDDEN_LABEL_CHARACTERS: &str = "\"*+,./:;<=>?[\\]|";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum Problem {
    #[error("the contest must end after it starts")]
    ContestEndsBeforeItStarts,
    #[error("the computer name pattern is empty")]
    ComputerNameEmpty,
    #[error(
        "the computer name pattern has an unknown placeholder '{0}' (use {{team}}, {{site}} or {{n}})"
    )]
    ComputerNameUnknownPlaceholder(String),
    #[error(
        "the computer name pattern may only have letters, digits and '-' besides placeholders (found '{0}')"
    )]
    ComputerNameBadCharacter(char),
    #[error("the USB label is empty")]
    UsbLabelEmpty,
    #[error("the USB label has {0} characters, at most {MAX_USB_LABEL_LENGTH} are allowed")]
    UsbLabelTooLong(usize),
    #[error("the USB label cannot contain '{0}'")]
    UsbLabelBadCharacter(char),
    #[error("the wallpaper '{0}' must be a file name, not a path")]
    WallpaperNotAFileName(String),
    #[error("a bookmark for {0} has no title")]
    BookmarkWithoutTitle(String),
    #[error(
        "the bookmark '{title}' needs an address starting with http:// or https:// (got '{url}')"
    )]
    BookmarkUrlInvalid { title: String, url: String },
    #[error("the home page needs an address starting with http:// or https:// (got '{0}')")]
    HomePageInvalid(String),
    #[error("'{0}' is not a valid site name")]
    SiteNameInvalid(String),
    #[error("the snapshot interval must be at least 1 second")]
    SnapshotIntervalZero,
    #[error("the recording retention must be at least 1 day")]
    RetentionDaysZero,
    #[error("the admin password hash is not a crypt hash like $6$salt$hash")]
    AdminPasswordHashInvalid,
    #[error("the server address must start with http:// or https:// (got '{0}')")]
    ApiUrlInvalid(String),
    #[error("the heartbeat interval must be at least 1 second")]
    HeartbeatIntervalZero,
    #[error(
        "the {0} must be 1 to {MAX_IDENTITY_LENGTH} characters, without line breaks or other control characters"
    )]
    IdentityTextInvalid(&'static str),
    #[error(
        "the settings file has format version {0}, this program reads versions {LEGACY_SCHEMA} to {SCHEMA}"
    )]
    UnsupportedSchema(u32),
    #[error("the Wi-Fi name must be 1 to {MAX_SSID_BYTES} bytes, without control characters")]
    WifiSsidInvalid,
    #[error(
        "the Wi-Fi password must be {MIN_PASSWORD_LENGTH} to {MAX_PASSWORD_LENGTH} plain ASCII characters"
    )]
    WifiPasswordInvalid,
    #[error("the machine number must be 1 or more")]
    MachineNumberZero,
    #[error("the stick token is empty")]
    TokenEmpty,
    #[error("the computer name would be empty, so the pattern and the team need letters or digits")]
    ComputerNameUnusable,
}

impl Settings {
    /// An empty list means the settings can be used.
    #[must_use]
    pub fn validate(&self) -> Vec<Problem> {
        let mut problems = Vec::new();

        if let (Some(start), Some(end)) = (self.contest.start, self.contest.end)
            && end <= start
        {
            problems.push(Problem::ContestEndsBeforeItStarts);
        }

        for (field, text) in [("team", &self.team), ("site", &self.site)] {
            if !is_valid_identity_text(text) {
                problems.push(Problem::IdentityTextInvalid(field));
            }
        }

        check_branding(&self.branding, &mut problems);

        for site in &self.network.allowed_domains {
            if !is_valid_domain(site) {
                problems.push(Problem::SiteNameInvalid(site.clone()));
            }
        }

        if self.recording.snapshot_interval_s == Some(0) {
            problems.push(Problem::SnapshotIntervalZero);
        }

        if self.recording.retention_days == Some(0) {
            problems.push(Problem::RetentionDaysZero);
        }

        if let Some(hash) = &self.admin_password_hash
            && !is_valid_password_hash(hash)
        {
            // The hash is not copied into the problem.
            problems.push(Problem::AdminPasswordHashInvalid);
        }

        if !self.api.url.is_empty() && !is_valid_http_url(&self.api.url) {
            problems.push(Problem::ApiUrlInvalid(self.api.url.clone()));
        }
        if self.api.heartbeat_s == 0 {
            problems.push(Problem::HeartbeatIntervalZero);
        }

        if let Some(wifi) = &self.wifi {
            if !is_valid_ssid(&wifi.ssid) {
                problems.push(Problem::WifiSsidInvalid);
            }
            // The password is never copied into a problem, so it cannot end up in a message.
            if let Some(password) = &wifi.password
                && !is_valid_password(password.expose())
            {
                problems.push(Problem::WifiPasswordInvalid);
            }
        }

        problems
    }
}

impl StickConfig {
    #[must_use]
    pub fn validate(&self) -> Vec<Problem> {
        let mut problems = Vec::new();
        if !(LEGACY_SCHEMA..=SCHEMA).contains(&self.schema) {
            problems.push(Problem::UnsupportedSchema(self.schema));
        }
        if self.machine_number == Some(0) {
            problems.push(Problem::MachineNumberZero);
        }
        // HMAC takes an empty key, so an empty token makes every signature forgeable.
        if self.token.expose().is_empty() {
            problems.push(Problem::TokenEmpty);
        }
        problems.extend(self.settings.validate());
        // The pattern may be fine on its own and still give nothing for this team and site.
        if self.computer_name().is_empty() {
            problems.push(Problem::ComputerNameUnusable);
        }
        problems
    }
}

/// A crypt hash: `$id$...` with no spaces, colons or control characters, so it fits one field of
/// the `chpasswd -e` input. Plain text and empty values are refused.
fn is_valid_password_hash(hash: &str) -> bool {
    let mut parts = hash.split('$');
    let starts_with_dollar = parts.next() == Some("");
    let has_id = parts.next().is_some_and(|id| !id.is_empty());
    // `$6$` or `$6$$` would pass chpasswd and then no password would ever match.
    let has_salt_and_hash = parts.next().is_some_and(|salt| !salt.is_empty())
        && parts.next().is_some_and(|hash| !hash.is_empty());
    starts_with_dollar
        && has_id
        && has_salt_and_hash
        && hash.len() <= 256
        && hash.chars().all(|c| c.is_ascii_graphic() && c != ':')
}

fn is_valid_identity_text(text: &str) -> bool {
    let length = text.trim().chars().count();
    (1..=MAX_IDENTITY_LENGTH).contains(&length) && !text.chars().any(char::is_control)
}

fn check_branding(branding: &Branding, problems: &mut Vec<Problem>) {
    check_computer_name_pattern(&branding.computer_name_pattern, problems);
    check_usb_label(&branding.usb_label, problems);

    if let Some(wallpaper) = &branding.wallpaper
        && (wallpaper.is_empty()
            // A colon makes a drive-relative path or an alternate data stream on Windows, and a
            // control character can hide the real name.
            || wallpaper.contains(['/', '\\', ':'])
            || wallpaper.chars().any(char::is_control)
            || wallpaper == "."
            || wallpaper == "..")
    {
        problems.push(Problem::WallpaperNotAFileName(wallpaper.clone()));
    }

    for bookmark in &branding.bookmarks {
        if bookmark.title.trim().is_empty() {
            problems.push(Problem::BookmarkWithoutTitle(bookmark.url.clone()));
        }
        if !is_valid_http_url(&bookmark.url) {
            problems.push(Problem::BookmarkUrlInvalid {
                title: bookmark.title.clone(),
                url: bookmark.url.clone(),
            });
        }
    }

    if let Some(home_page) = &branding.home_page
        && !is_valid_http_url(home_page)
    {
        problems.push(Problem::HomePageInvalid(home_page.clone()));
    }
}

fn check_computer_name_pattern(pattern: &str, problems: &mut Vec<Problem>) {
    if pattern.is_empty() {
        problems.push(Problem::ComputerNameEmpty);
        return;
    }

    let mut rest = pattern;
    while !rest.is_empty() {
        if rest.starts_with('{') {
            let end = rest.find('}').map_or(rest.len(), |at| at + 1);
            let token = &rest[..end];
            if !COMPUTER_NAME_PLACEHOLDERS.contains(&token) {
                problems.push(Problem::ComputerNameUnknownPlaceholder(token.to_owned()));
            }
            rest = &rest[end..];
        } else {
            let mut characters = rest.chars();
            if let Some(character) = characters.next() {
                if !(character.is_ascii_alphanumeric() || character == '-') {
                    problems.push(Problem::ComputerNameBadCharacter(character));
                }
                rest = characters.as_str();
            }
        }
    }
}

fn check_usb_label(label: &str, problems: &mut Vec<Problem>) {
    if label.is_empty() {
        problems.push(Problem::UsbLabelEmpty);
        return;
    }
    let length = label.chars().count();
    if length > MAX_USB_LABEL_LENGTH {
        problems.push(Problem::UsbLabelTooLong(length));
    }
    if let Some(bad) = label
        .chars()
        .find(|c| !c.is_ascii() || c.is_ascii_control() || FORBIDDEN_LABEL_CHARACTERS.contains(*c))
    {
        problems.push(Problem::UsbLabelBadCharacter(bad));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::branding::Bookmark;
    use crate::ids::{StickId, Token};

    const NO_PROBLEMS: [Problem; 0] = [];

    fn problems_with(change: impl FnOnce(&mut Settings)) -> Vec<Problem> {
        let mut settings = Settings::default();
        change(&mut settings);
        settings.validate()
    }

    #[test]
    fn the_defaults_have_no_problems() {
        assert_eq!(Settings::default().validate(), NO_PROBLEMS);
    }

    #[test]
    fn team_and_site_are_short_single_line_text() {
        let team = |text: &str| problems_with(|s| s.team = text.to_owned());
        assert_eq!(team("Team A"), NO_PROBLEMS);
        assert_eq!(team("   "), [Problem::IdentityTextInvalid("team")]);
        assert_eq!(team("a\nb"), [Problem::IdentityTextInvalid("team")]);
        assert_eq!(
            team(&"x".repeat(65)),
            [Problem::IdentityTextInvalid("team")]
        );
        let site = |text: &str| problems_with(|s| s.site = text.to_owned());
        assert_eq!(site("Lab\r1"), [Problem::IdentityTextInvalid("site")]);
    }

    #[test]
    fn a_contest_must_end_after_it_starts() {
        let problems = problems_with(|s| {
            s.contest.start = Some("2026-11-15T14:00".parse().expect("valid time"));
            s.contest.end = Some("2026-11-15T09:00".parse().expect("valid time"));
        });
        assert_eq!(problems, [Problem::ContestEndsBeforeItStarts]);
    }

    #[test]
    fn checks_the_computer_name_pattern() {
        let pattern =
            |text: &str| problems_with(|s| s.branding.computer_name_pattern = text.to_owned());
        assert_eq!(pattern("{site}-{team}-{n}"), NO_PROBLEMS);
        assert_eq!(pattern(""), [Problem::ComputerNameEmpty]);
        assert_eq!(
            pattern("{team}-{seat}"),
            [Problem::ComputerNameUnknownPlaceholder("{seat}".to_owned())]
        );
        assert_eq!(pattern("pc_{n}"), [Problem::ComputerNameBadCharacter('_')]);
    }

    #[test]
    fn checks_the_usb_label() {
        let label = |text: &str| problems_with(|s| s.branding.usb_label = text.to_owned());
        assert_eq!(label("ELEVENCHARS"), NO_PROBLEMS);
        assert_eq!(label(""), [Problem::UsbLabelEmpty]);
        assert_eq!(label("TWELVE CHARS"), [Problem::UsbLabelTooLong(12)]);
        assert_eq!(label("A:B"), [Problem::UsbLabelBadCharacter(':')]);
    }

    #[test]
    fn checks_files_and_web_addresses() {
        let problems = problems_with(|s| {
            s.branding.wallpaper = Some("../wall.png".to_owned());
            s.branding.bookmarks = vec![Bookmark {
                title: "Bad".to_owned(),
                url: "judge.example.org".to_owned(),
            }];
            s.branding.home_page = Some("javascript:alert(1)".to_owned());
            s.network.allowed_domains = vec!["not a site".to_owned()];
            s.api.url = "ftp://x.org".to_owned();
        });
        assert_eq!(problems.len(), 5);
    }

    #[test]
    fn a_wallpaper_name_cannot_reach_another_drive_or_stream() {
        for name in [
            "C:wall.png",
            "wall.png:stream",
            "wall\n.png",
            "wall\u{0}.png",
        ] {
            let problems = problems_with(|s| s.branding.wallpaper = Some(name.to_owned()));
            assert_eq!(
                problems,
                [Problem::WallpaperNotAFileName(name.to_owned())],
                "{name:?}"
            );
        }
    }

    #[test]
    fn checks_the_wifi_settings_without_repeating_the_password() {
        use crate::wifi::{Wifi, WifiPassword};
        let wifi = |ssid: &str, password: Option<&str>| {
            problems_with(|s| {
                s.wifi = Some(Wifi {
                    ssid: ssid.to_owned(),
                    password: password.map(WifiPassword::new),
                });
            })
        };
        assert_eq!(wifi("venue", Some("12345678")), NO_PROBLEMS);
        assert_eq!(wifi("open", None), NO_PROBLEMS);
        assert_eq!(wifi("", None), [Problem::WifiSsidInvalid]);
        assert_eq!(wifi("venue", Some("short")), [Problem::WifiPasswordInvalid]);
        let message = Problem::WifiPasswordInvalid.to_string();
        assert!(!message.contains("short"));
    }

    #[test]
    fn the_retention_is_at_least_one_day() {
        assert_eq!(
            problems_with(|s| s.recording.retention_days = Some(0)),
            [Problem::RetentionDaysZero]
        );
        assert_eq!(
            problems_with(|s| s.recording.retention_days = Some(30)),
            NO_PROBLEMS
        );
    }

    #[test]
    fn the_admin_password_must_be_a_hash() {
        let hash = |text: &str| problems_with(|s| s.admin_password_hash = Some(text.to_owned()));
        assert_eq!(hash("$6$salt$abcdef"), NO_PROBLEMS);
        assert_eq!(hash("$y$j9T$salt$abcdef"), NO_PROBLEMS);
        for bad in [
            "",
            "plain-password",
            "$",
            "$6",
            "$$x",
            "$6$",
            "$6$$",
            "$6$salt$",
            "$6$a b$c",
            "$6$a:b$c",
        ] {
            assert_eq!(hash(bad), [Problem::AdminPasswordHashInvalid], "{bad:?}");
        }
        assert_eq!(problems_with(|_| {}), NO_PROBLEMS);
    }

    #[test]
    fn a_stick_file_needs_the_current_format() {
        let mut config = StickConfig {
            schema: SCHEMA,
            stick_id: StickId::parse("s1").expect("valid id"),
            created_at: "2026-11-10T15:22".parse().expect("valid time"),
            token: Token::new("t"),
            machine_number: None,
            settings: Settings::default(),
        };
        assert_eq!(config.validate(), NO_PROBLEMS);
        config.schema = LEGACY_SCHEMA;
        assert_eq!(
            config.validate(),
            NO_PROBLEMS,
            "the first format is still read"
        );
        config.schema = SCHEMA + 1;
        assert_eq!(config.validate(), [Problem::UnsupportedSchema(SCHEMA + 1)]);
        config.schema = 0;
        assert_eq!(config.validate(), [Problem::UnsupportedSchema(0)]);
        config.schema = SCHEMA;

        config.token = Token::new("");
        assert_eq!(config.validate(), [Problem::TokenEmpty]);
        config.token = Token::new("t");

        config.machine_number = Some(0);
        assert_eq!(config.validate(), [Problem::MachineNumberZero]);
        config.machine_number = Some(3);
        assert_eq!(config.validate(), NO_PROBLEMS);

        // Nothing usable is left of the name once the characters a host name cannot have are gone.
        config.settings.branding.computer_name_pattern = "{team}".to_owned();
        config.settings.team = "!!!".to_owned();
        assert_eq!(config.validate(), [Problem::ComputerNameUnusable]);
    }
}
