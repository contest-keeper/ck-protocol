use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// 1 to 64 ASCII letters, digits or `-`. Sent in the `X-CK-Stick` header.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StickId(String);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("a stick id has 1 to 64 letters, digits or '-' (got '{0}')")]
pub struct InvalidStickId(String);

impl StickId {
    const MAX_LENGTH: usize = 64;

    pub fn parse(text: &str) -> Result<Self, InvalidStickId> {
        let valid = !text.is_empty()
            && text.len() <= Self::MAX_LENGTH
            && text.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
        if valid {
            Ok(Self(text.to_owned()))
        } else {
            Err(InvalidStickId(text.to_owned()))
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StickId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Serialize for StickId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for StickId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(serde::de::Error::custom)
    }
}

/// The stick's secret. It has no `Display` and its `Debug` hides the value, so it cannot reach a
/// log by accident.
#[derive(Clone, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Token(String);

/// Compares every byte, so the time taken does not reveal where two secrets first differ.
/// (The length is not secret, so a different length can return early.)
impl PartialEq for Token {
    fn eq(&self, other: &Self) -> bool {
        let (a, b) = (self.0.as_bytes(), other.0.as_bytes());
        if a.len() != b.len() {
            return false;
        }
        let mut difference = 0_u8;
        for (x, y) in a.iter().zip(b) {
            difference |= x ^ y;
        }
        difference == 0
    }
}

impl Eq for Token {}

impl Token {
    #[must_use]
    pub fn new(secret: impl Into<String>) -> Self {
        Self(secret.into())
    }

    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Token(hidden)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_stick_ids_when_they_are_read() {
        assert!(serde_json::from_str::<StickId>("\"03c9f556\"").is_ok());
        for text in ["", "has space", "slash/", &"x".repeat(65)] {
            assert!(StickId::parse(text).is_err(), "{text:?} should be rejected");
        }
        assert!(serde_json::from_str::<StickId>("\"not valid!\"").is_err());
    }

    #[test]
    fn tokens_compare_by_value() {
        assert_eq!(Token::new("secret"), Token::new("secret"));
        assert_ne!(Token::new("secret"), Token::new("secreT"));
        assert_ne!(Token::new("secret"), Token::new("secret2"));
        assert_ne!(Token::new(""), Token::new("x"));
    }

    #[test]
    fn the_token_does_not_show_in_debug_output() {
        assert!(!format!("{:?}", Token::new("super-secret")).contains("super-secret"));
    }
}
