//! Which sites a contestant may open.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Network {
    /// `example.org`, or `.example.org` to include subdomains.
    pub allowed_domains: Vec<String>,
}

/// At least two parts of ASCII letters, digits and `-`, with an optional leading dot.
#[must_use]
pub fn is_valid_domain(domain: &str) -> bool {
    let name = domain.strip_prefix('.').unwrap_or(domain);
    let parts: Vec<&str> = name.split('.').collect();
    parts.len() >= 2
        && parts.iter().all(|part| {
            !part.is_empty()
                && !part.starts_with('-')
                && !part.ends_with('-')
                && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_site_names() {
        for name in ["example.org", ".rcpc.kilonova.ro", "a-b.co", "x1.y2.z3"] {
            assert!(is_valid_domain(name), "{name} should be valid");
        }
    }

    #[test]
    fn rejects_other_text() {
        for name in [
            "",
            "localhost",
            ".",
            "..org",
            "bad domain.com",
            "-a.com",
            "a-.com",
            "a..com",
            "a.com/",
            "http://a.com",
        ] {
            assert!(!is_valid_domain(name), "{name:?} should be invalid");
        }
    }

    #[test]
    fn no_site_is_allowed_until_the_admin_chooses() {
        assert_eq!(Network::default().allowed_domains, Vec::<String>::new());
    }
}
