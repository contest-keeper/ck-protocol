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
    // No "user:password@host": credentials must never travel in an address, and they would be
    // copied into error messages.
    !host.is_empty()
        && !host.starts_with(':')
        && !host.contains('@')
        && !url.chars().any(|c| c.is_whitespace() || c.is_control())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_web_addresses_and_rejects_other_text() {
        for url in [
            "http://10.0.2.2:8088",
            "https://hub.example.org",
            "https://a.b/c?d#e",
        ] {
            assert!(is_valid_http_url(url), "{url} should be valid");
        }
        for url in [
            "",
            "hub.example.org",
            "ftp://x.org",
            "https://",
            "http://:80",
            "https://a b.org",
            "https://user:secret@hub.example.org",
        ] {
            assert!(!is_valid_http_url(url), "{url:?} should be invalid");
        }
    }
}
