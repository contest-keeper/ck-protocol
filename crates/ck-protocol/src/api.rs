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

/// The Hub address of a stick: `https://`, or `http://` only to this machine. A stick sends its
/// enrolment code and its reports there, and the agent refuses plain http to anything else.
#[must_use]
pub fn is_valid_hub_url(url: &str) -> bool {
    if !is_valid_http_url(url) {
        return false;
    }
    let Some(rest) = url.strip_prefix("http://") else {
        return true;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = match authority.strip_prefix('[') {
        Some(ipv6) => ipv6.split(']').next().unwrap_or_default(),
        None => authority.split(':').next().unwrap_or_default(),
    };
    host.eq_ignore_ascii_case("localhost") || host == "127.0.0.1" || host == "::1"
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

    #[test]
    fn a_hub_address_is_https_or_plain_http_to_this_machine() {
        for url in [
            "https://hub.example.org",
            "http://localhost:8088",
            "http://127.0.0.1:8088/x",
            "http://[::1]:8088",
        ] {
            assert!(is_valid_hub_url(url), "{url} should be valid");
        }
        for url in [
            "http://10.0.2.2:8088",
            "http://hub.example.org",
            "http://localhost.evil.org",
            "http://127.0.0.1.evil.org",
            "ftp://localhost",
        ] {
            assert!(!is_valid_hub_url(url), "{url:?} should be invalid");
        }
    }
}
