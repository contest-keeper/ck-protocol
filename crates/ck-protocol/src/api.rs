//! Where the stick reports to.

use serde::{Deserialize, Serialize};

/// The stick's secret is not here; it belongs to the stick (see `StickConfig`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Api {
    /// Base URL of the Hub. Empty means no reporting.
    pub url: String,
    pub heartbeat_s: u64,
}

impl Default for Api {
    fn default() -> Self {
        Self {
            url: String::new(),
            heartbeat_s: 15,
        }
    }
}

#[must_use]
pub fn is_valid_http_url(url: &str) -> bool {
    let Some(rest) = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))
    else {
        return false;
    };
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    !host.is_empty()
        && !host.starts_with(':')
        && !url.chars().any(|c| c.is_whitespace() || c.is_control())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_http_and_https_addresses() {
        for url in [
            "http://10.0.2.2:8088",
            "https://hub.example.org",
            "https://a.b/c?d#e",
        ] {
            assert!(is_valid_http_url(url), "{url} should be valid");
        }
    }

    #[test]
    fn rejects_other_text() {
        for url in [
            "",
            "hub.example.org",
            "ftp://x.org",
            "https://",
            "http://:80",
            "https://a b.org",
            "https:///x",
        ] {
            assert!(!is_valid_http_url(url), "{url:?} should be invalid");
        }
    }

    #[test]
    fn defaults_report_nowhere_every_15_seconds() {
        let api = Api::default();
        assert_eq!(api.url, "");
        assert_eq!(api.heartbeat_s, 15);
    }
}
