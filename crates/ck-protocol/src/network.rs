use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Network {
    /// `example.org`, or `.example.org` to include subdomains.
    pub allowed_domains: Vec<String>,
}

/// At least two parts of ASCII letters, digits and `-` (63 characters each at most, 253 in all),
/// with an optional leading dot.
#[must_use]
pub fn is_valid_domain(domain: &str) -> bool {
    let name = domain.strip_prefix('.').unwrap_or(domain);
    let parts: Vec<&str> = name.split('.').collect();
    parts.len() >= 2
        && name.len() <= 253
        && parts.iter().all(|part| {
            !part.is_empty()
                && part.len() <= 63
                && !part.starts_with('-')
                && !part.ends_with('-')
                && part.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_site_names_and_rejects_other_text() {
        for name in ["example.org", ".judge.example.org", "x1.y2.z3"] {
            assert!(is_valid_domain(name), "{name} should be valid");
        }
        let long_label = format!("{}.org", "a".repeat(64));
        assert!(!is_valid_domain(&long_label));
        for name in [
            "",
            "localhost",
            "..org",
            "bad domain.com",
            "-a.com",
            "a.com/",
            "http://a.com",
        ] {
            assert!(!is_valid_domain(name), "{name:?} should be invalid");
        }
    }
}
