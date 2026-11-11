//! Identifiers and secrets of a stick.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// The identity of one stick, made by the admin tool, such as `03c9f556`.
///
/// It is 1 to 64 characters: ASCII letters, digits and `-`. It travels in the `X-CK-Stick` header.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StickId(String);

/// The text is not a valid stick id.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("a stick id has 1 to 64 letters, digits or '-' (got '{0}')")]
pub struct InvalidStickId(String);

impl StickId {
    const MAX_LENGTH: usize = 64;

    /// Checks the text once; after that the type can be trusted.
    ///
    /// # Errors
    /// Returns an error when the text is empty, longer than 64 characters or has other characters.
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

    /// The id as text.
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

/// The secret of one stick. Reports are signed with it.
///
/// It has no `Display` and its `Debug` output is hidden, so it cannot end up in a log by accident.
/// Use [`Token::expose`] only where the secret is really needed, for example to sign.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Token(String);

impl Token {
    /// Wraps a secret.
    #[must_use]
    pub fn new(secret: impl Into<String>) -> Self {
        Self(secret.into())
    }

    /// The secret itself. Never log it.
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
    fn accepts_ordinary_stick_ids() {
        for text in ["03c9f556", "A", "lab-1-seat-07", &"x".repeat(64)] {
            assert_eq!(StickId::parse(text).expect(text).as_str(), text);
        }
    }

    #[test]
    fn rejects_bad_stick_ids() {
        for text in ["", "has space", "slash/", "dot.", "ünï", &"x".repeat(65)] {
            assert!(StickId::parse(text).is_err(), "{text:?} should be rejected");
        }
    }

    #[test]
    fn stick_id_is_checked_when_read_from_json() {
        assert!(serde_json::from_str::<StickId>("\"03c9f556\"").is_ok());
        assert!(serde_json::from_str::<StickId>("\"not valid!\"").is_err());
    }

    #[test]
    fn token_is_hidden_in_debug_output() {
        let token = Token::new("super-secret");
        assert!(!format!("{token:?}").contains("super-secret"));
        assert_eq!(token.expose(), "super-secret");
    }

    #[test]
    fn token_is_a_plain_string_in_json() {
        let token: Token = serde_json::from_str("\"abc\"").expect("valid json");
        assert_eq!(
            serde_json::to_string(&token).expect("serializes"),
            "\"abc\""
        );
    }
}
