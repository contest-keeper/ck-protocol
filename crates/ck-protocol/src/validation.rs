//! Checks on the settings, with problems described in plain words for the admin.

use crate::api::is_valid_http_url;
use crate::branding::{Branding, COMPUTER_NAME_PLACEHOLDERS};
use crate::network::is_valid_domain;
use crate::settings::{SCHEMA, Settings, StickConfig};

pub const MAX_USB_LABEL_LENGTH: usize = 11;

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
    #[error("the server address must start with http:// or https:// (got '{0}')")]
    ApiUrlInvalid(String),
    #[error("the heartbeat interval must be at least 1 second")]
    HeartbeatIntervalZero,
    #[error("the settings file has format version {0}, this program reads version {SCHEMA}")]
    UnsupportedSchema(u32),
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

        check_branding(&self.branding, &mut problems);

        problems.extend(
            self.network
                .allowed_domains
                .iter()
                .filter(|site| !is_valid_domain(site))
                .map(|site| Problem::SiteNameInvalid(site.clone())),
        );

        if self.recording.snapshot_interval_s == Some(0) {
            problems.push(Problem::SnapshotIntervalZero);
        }

        if !self.api.url.is_empty() && !is_valid_http_url(&self.api.url) {
            problems.push(Problem::ApiUrlInvalid(self.api.url.clone()));
        }
        if self.api.heartbeat_s == 0 {
            problems.push(Problem::HeartbeatIntervalZero);
        }

        problems
    }
}

impl StickConfig {
    #[must_use]
    pub fn validate(&self) -> Vec<Problem> {
        let mut problems = Vec::new();
        if self.schema != SCHEMA {
            problems.push(Problem::UnsupportedSchema(self.schema));
        }
        problems.extend(self.settings.validate());
        problems
    }
}

fn check_branding(branding: &Branding, problems: &mut Vec<Problem>) {
    check_computer_name_pattern(&branding.computer_name_pattern, problems);
    check_usb_label(&branding.usb_label, problems);

    if let Some(wallpaper) = &branding.wallpaper
        && (wallpaper.is_empty()
            || wallpaper.contains(['/', '\\'])
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

    const NO_PROBLEMS: [Problem; 0] = [];

    fn problems_with(change: impl FnOnce(&mut Settings)) -> Vec<Problem> {
        let mut settings = Settings::default();
        change(&mut settings);
        settings.validate()
    }

    #[test]
    fn the_defaults_have_no_problems() {
        assert_eq!(Settings::default().validate(), Vec::new());
    }

    #[test]
    fn rejects_a_contest_that_ends_before_it_starts() {
        let problems = problems_with(|s| {
            s.contest.start = Some("2026-11-15T14:00".parse().expect("valid time"));
            s.contest.end = Some("2026-11-15T09:00".parse().expect("valid time"));
        });
        assert_eq!(problems, [Problem::ContestEndsBeforeItStarts]);
    }

    #[test]
    fn accepts_a_contest_with_only_a_start() {
        let problems = problems_with(|s| {
            s.contest.start = Some("2026-11-15T09:00".parse().expect("valid time"));
        });
        assert_eq!(problems, NO_PROBLEMS);
    }

    #[test]
    fn rejects_an_empty_computer_name_pattern() {
        assert_eq!(
            problems_with(|s| s.branding.computer_name_pattern.clear()),
            [Problem::ComputerNameEmpty]
        );
    }

    #[test]
    fn rejects_unknown_placeholders_in_the_computer_name() {
        let problems =
            problems_with(|s| s.branding.computer_name_pattern = "{team}-{seat}".to_owned());
        assert_eq!(
            problems,
            [Problem::ComputerNameUnknownPlaceholder("{seat}".to_owned())]
        );
        let unclosed = problems_with(|s| s.branding.computer_name_pattern = "pc-{team".to_owned());
        assert_eq!(
            unclosed,
            [Problem::ComputerNameUnknownPlaceholder("{team".to_owned())]
        );
    }

    #[test]
    fn rejects_bad_characters_in_the_computer_name() {
        let problems = problems_with(|s| s.branding.computer_name_pattern = "pc_{n} x".to_owned());
        assert_eq!(
            problems,
            [
                Problem::ComputerNameBadCharacter('_'),
                Problem::ComputerNameBadCharacter(' ')
            ]
        );
    }

    #[test]
    fn accepts_every_placeholder_together() {
        assert_eq!(
            problems_with(|s| s.branding.computer_name_pattern = "{site}-{team}-{n}".to_owned()),
            NO_PROBLEMS
        );
    }

    #[test]
    fn checks_the_usb_label() {
        assert_eq!(
            problems_with(|s| s.branding.usb_label.clear()),
            [Problem::UsbLabelEmpty]
        );
        assert_eq!(
            problems_with(|s| s.branding.usb_label = "ELEVENCHARS".to_owned()),
            NO_PROBLEMS
        );
        assert_eq!(
            problems_with(|s| s.branding.usb_label = "TWELVE CHARS".to_owned()),
            [Problem::UsbLabelTooLong(12)]
        );
        assert_eq!(
            problems_with(|s| s.branding.usb_label = "A:B".to_owned()),
            [Problem::UsbLabelBadCharacter(':')]
        );
        assert_eq!(
            problems_with(|s| s.branding.usb_label = "CONCURS\u{103}".to_owned()),
            [Problem::UsbLabelBadCharacter('\u{103}')]
        );
    }

    #[test]
    fn the_wallpaper_must_be_a_file_name() {
        assert_eq!(
            problems_with(|s| s.branding.wallpaper = Some("wall.png".to_owned())),
            NO_PROBLEMS
        );
        for path in ["a/b.png", "..\\b.png", "", ".."] {
            assert_eq!(
                problems_with(|s| s.branding.wallpaper = Some(path.to_owned())),
                [Problem::WallpaperNotAFileName(path.to_owned())],
                "{path:?}"
            );
        }
    }

    #[test]
    fn checks_bookmarks_and_the_home_page() {
        let problems = problems_with(|s| {
            s.branding.bookmarks = vec![
                Bookmark {
                    title: "Judge".to_owned(),
                    url: "https://rcpc.kilonova.ro".to_owned(),
                },
                Bookmark {
                    title: " ".to_owned(),
                    url: "https://ok.org".to_owned(),
                },
                Bookmark {
                    title: "Bad".to_owned(),
                    url: "kilonova.ro".to_owned(),
                },
            ];
            s.branding.home_page = Some("javascript:alert(1)".to_owned());
        });
        assert_eq!(
            problems,
            [
                Problem::BookmarkWithoutTitle("https://ok.org".to_owned()),
                Problem::BookmarkUrlInvalid {
                    title: "Bad".to_owned(),
                    url: "kilonova.ro".to_owned()
                },
                Problem::HomePageInvalid("javascript:alert(1)".to_owned()),
            ]
        );
    }

    #[test]
    fn rejects_bad_site_names() {
        let problems = problems_with(|s| s.network.allowed_domains.push("not a site".to_owned()));
        assert_eq!(
            problems,
            [Problem::SiteNameInvalid("not a site".to_owned())]
        );
    }

    #[test]
    fn rejects_zero_intervals() {
        assert_eq!(
            problems_with(|s| s.recording.snapshot_interval_s = Some(0)),
            [Problem::SnapshotIntervalZero]
        );
        assert_eq!(
            problems_with(|s| s.api.heartbeat_s = 0),
            [Problem::HeartbeatIntervalZero]
        );
    }

    #[test]
    fn rejects_a_server_address_that_is_not_a_web_address() {
        assert_eq!(
            problems_with(|s| s.api.url = "ftp://x.org".to_owned()),
            [Problem::ApiUrlInvalid("ftp://x.org".to_owned())]
        );
        assert_eq!(
            problems_with(|s| s.api.url = "https://hub.example.org".to_owned()),
            NO_PROBLEMS
        );
    }

    #[test]
    fn reports_every_problem_at_once() {
        let problems = problems_with(|s| {
            s.api.heartbeat_s = 0;
            s.api.url = "nope".to_owned();
            s.branding.usb_label.clear();
        });
        assert_eq!(problems.len(), 3);
    }

    #[test]
    fn problems_read_as_plain_sentences() {
        let text = Problem::UsbLabelTooLong(12).to_string();
        assert_eq!(
            text,
            "the USB label has 12 characters, at most 11 are allowed"
        );
        let text = Problem::ComputerNameUnknownPlaceholder("{seat}".to_owned()).to_string();
        assert!(text.contains("{seat}") && text.contains("{team}"));
    }

    #[test]
    fn a_stick_file_needs_the_current_format() {
        use crate::ids::{StickId, Token};
        let mut config = StickConfig {
            schema: SCHEMA,
            stick_id: StickId::parse("s1").expect("valid id"),
            created_at: "2026-11-10T15:22".parse().expect("valid time"),
            token: Token::new("t"),
            settings: Settings::default(),
        };
        assert_eq!(config.validate(), NO_PROBLEMS);
        config.schema = 2;
        assert_eq!(config.validate(), [Problem::UnsupportedSchema(2)]);
    }
}
