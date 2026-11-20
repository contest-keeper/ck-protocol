//! Signed stick settings.
//!
//! The settings file of a stick (`config.json` on the CKCONFIG partition) can be edited by anyone
//! who holds the stick, on any PC. So the Hub signs it: Forge sends the exact bytes it is about to
//! write, and the Hub returns an Ed25519 signature that goes next to the file. The image carries the
//! public key, and the stick refuses a settings file whose signature does not match its bytes.
//!
//! The private key never leaves the Hub. A signature only proves that the Hub signed these bytes;
//! what the stick does with a refused file is up to the agent.

use std::fmt;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use ed25519_dalek::{Signature, Signer as _, SigningKey, VerifyingKey};
use sha2::{Digest, Sha256};

/// The signature file, written next to `config.json`.
pub const CONFIG_SIGNATURE_FILE: &str = "config.json.sig";

/// Keeps a signature made for settings from being valid for anything else the same key might sign.
const DOMAIN: &[u8] = b"keeper-os stick settings v1\n";

/// The first word of a signature file, which also says its format.
const SIGNATURE_WORD: &str = "ck-settings-signature-v1";

const KEY_BYTES: usize = 32;
const SIGNATURE_BYTES: usize = 64;

/// Why a key or a signature could not be read or did not match.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignatureError {
    /// A key that is not 32 bytes of base64.
    NotAKey,
    /// A signature file that is not in the format of [`ConfigSignature::to_file_text`].
    NotASignature,
    /// The signature names a key that is not in the list.
    UnknownKey,
    /// The signature does not match the settings.
    Mismatch,
}

impl fmt::Display for SignatureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NotAKey => "not a signing key",
            Self::NotASignature => "not a settings signature",
            Self::UnknownKey => "signed with a key this image does not know",
            Self::Mismatch => "the signature does not match the settings",
        })
    }
}

impl std::error::Error for SignatureError {}

/// The private key the Hub signs settings with. It has no `Debug` output of its key.
pub struct ConfigSigningKey(SigningKey);

impl fmt::Debug for ConfigSigningKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ConfigSigningKey(..)")
    }
}

impl ConfigSigningKey {
    /// A new random key, for the tool that makes the Hub's key.
    #[must_use]
    pub fn generate() -> Self {
        let mut seed = [0_u8; KEY_BYTES];
        rand::fill(&mut seed);
        Self(SigningKey::from_bytes(&seed))
    }

    /// Reads the key from the base64 of its 32-byte seed, as [`Self::seed_base64`] writes it.
    ///
    /// # Errors
    /// [`SignatureError::NotAKey`] for anything else.
    pub fn from_seed_base64(text: &str) -> Result<Self, SignatureError> {
        let seed: [u8; KEY_BYTES] = STANDARD
            .decode(text.trim())
            .ok()
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or(SignatureError::NotAKey)?;
        Ok(Self(SigningKey::from_bytes(&seed)))
    }

    /// The secret seed as base64. Only for the tool that makes the key.
    #[must_use]
    pub fn seed_base64(&self) -> String {
        STANDARD.encode(self.0.to_bytes())
    }

    #[must_use]
    pub fn verifying_key(&self) -> ConfigVerifyingKey {
        ConfigVerifyingKey(self.0.verifying_key())
    }

    /// Signs the exact bytes of a settings file.
    #[must_use]
    pub fn sign(&self, config: &[u8]) -> ConfigSignature {
        let signature = self.0.sign(&message(config));
        ConfigSignature {
            key_id: self.verifying_key().key_id(),
            bytes: signature.to_bytes(),
        }
    }
}

/// A public key an image trusts to sign settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigVerifyingKey(VerifyingKey);

impl ConfigVerifyingKey {
    /// Reads a key from the base64 of its 32 bytes.
    ///
    /// # Errors
    /// [`SignatureError::NotAKey`] for anything else.
    pub fn from_base64(text: &str) -> Result<Self, SignatureError> {
        let bytes: [u8; KEY_BYTES] = STANDARD
            .decode(text.trim())
            .ok()
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or(SignatureError::NotAKey)?;
        VerifyingKey::from_bytes(&bytes)
            .map(Self)
            .map_err(|_| SignatureError::NotAKey)
    }

    #[must_use]
    pub fn to_base64(&self) -> String {
        STANDARD.encode(self.0.to_bytes())
    }

    /// A short name for the key, so a signature says which key made it and a list of keys can
    /// change over time: 16 hex digits of the SHA-256 of the key.
    #[must_use]
    pub fn key_id(&self) -> String {
        hex::encode(&Sha256::digest(self.0.to_bytes())[..8])
    }
}

/// Reads the keys an image trusts: one base64 key per line, blank lines and `#` comments ignored.
///
/// # Errors
/// [`SignatureError::NotAKey`] for a line that is not a key.
pub fn parse_verifying_keys(text: &str) -> Result<Vec<ConfigVerifyingKey>, SignatureError> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(ConfigVerifyingKey::from_base64)
        .collect()
}

/// What the signature file holds: the key that signed and the signature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigSignature {
    key_id: String,
    bytes: [u8; SIGNATURE_BYTES],
}

impl ConfigSignature {
    #[must_use]
    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    /// The text of the signature file.
    #[must_use]
    pub fn to_file_text(&self) -> String {
        format!(
            "{SIGNATURE_WORD} {} {}\n",
            self.key_id,
            STANDARD.encode(self.bytes)
        )
    }

    /// Reads the text of a signature file.
    ///
    /// # Errors
    /// [`SignatureError::NotASignature`] for anything else.
    pub fn parse(text: &str) -> Result<Self, SignatureError> {
        let mut words = text.split_whitespace();
        let (Some(word), Some(key_id), Some(signature), None) =
            (words.next(), words.next(), words.next(), words.next())
        else {
            return Err(SignatureError::NotASignature);
        };
        let bytes: [u8; SIGNATURE_BYTES] = STANDARD
            .decode(signature)
            .ok()
            .and_then(|bytes| bytes.try_into().ok())
            .ok_or(SignatureError::NotASignature)?;
        let key_id_is_hex = key_id.len() == 16 && key_id.bytes().all(|b| b.is_ascii_hexdigit());
        if word != SIGNATURE_WORD || !key_id_is_hex {
            return Err(SignatureError::NotASignature);
        }
        Ok(Self {
            key_id: key_id.to_ascii_lowercase(),
            bytes,
        })
    }
}

/// Checks the text of a signature file against the exact bytes of a settings file, with the keys an
/// image trusts.
///
/// # Errors
/// [`SignatureError::NotASignature`] for a signature file that cannot be read,
/// [`SignatureError::UnknownKey`] when no trusted key made it, and [`SignatureError::Mismatch`]
/// when the settings are not the signed ones.
pub fn verify_config(
    keys: &[ConfigVerifyingKey],
    config: &[u8],
    signature_file: &str,
) -> Result<(), SignatureError> {
    let signature = ConfigSignature::parse(signature_file)?;
    let key = keys
        .iter()
        .find(|key| key.key_id() == signature.key_id)
        .ok_or(SignatureError::UnknownKey)?;
    key.0
        .verify_strict(&message(config), &Signature::from_bytes(&signature.bytes))
        .map_err(|_| SignatureError::Mismatch)
}

fn message(config: &[u8]) -> Vec<u8> {
    let mut message = Vec::with_capacity(DOMAIN.len() + config.len());
    message.extend_from_slice(DOMAIN);
    message.extend_from_slice(config);
    message
}

#[cfg(test)]
mod tests {
    use super::*;

    const SETTINGS: &[u8] = br#"{"schema":3,"stick_id":"03c9f556","team":"Team A"}"#;

    #[test]
    fn settings_signed_by_a_trusted_key_verify() {
        let key = ConfigSigningKey::generate();
        let file = key.sign(SETTINGS).to_file_text();
        assert_eq!(
            verify_config(&[key.verifying_key()], SETTINGS, &file),
            Ok(())
        );
    }

    #[test]
    fn any_change_to_the_settings_is_refused() {
        let key = ConfigSigningKey::generate();
        let file = key.sign(SETTINGS).to_file_text();
        let trusted = [key.verifying_key()];
        let text = std::str::from_utf8(SETTINGS).expect("settings are text");
        for edited in [
            text.replace("Team A", "Team B").into_bytes(),
            [SETTINGS, b" "].concat(),
            SETTINGS[1..].to_vec(),
            Vec::new(),
        ] {
            assert_eq!(
                verify_config(&trusted, &edited, &file),
                Err(SignatureError::Mismatch)
            );
        }
    }

    #[test]
    fn another_key_or_an_unreadable_signature_is_refused() {
        let key = ConfigSigningKey::generate();
        let other = ConfigSigningKey::generate();
        let file = other.sign(SETTINGS).to_file_text();
        assert_eq!(
            verify_config(&[key.verifying_key()], SETTINGS, &file),
            Err(SignatureError::UnknownKey)
        );
        for text in [
            "",
            "garbage",
            "ck-settings-signature-v1 0011223344556677",
            &file.replace("ck-settings-signature-v1", "other-word"),
            &format!("{} extra", file.trim()),
        ] {
            assert_eq!(
                verify_config(&[key.verifying_key()], SETTINGS, text),
                Err(SignatureError::NotASignature),
                "{text}"
            );
        }
    }

    #[test]
    fn a_signature_with_a_changed_byte_is_refused() {
        let key = ConfigSigningKey::generate();
        let text = key.sign(SETTINGS).to_file_text();
        let mut signature = ConfigSignature::parse(&text).expect("signature");
        signature.bytes[10] ^= 1;
        assert_eq!(
            verify_config(&[key.verifying_key()], SETTINGS, &signature.to_file_text()),
            Err(SignatureError::Mismatch)
        );
    }

    #[test]
    fn a_list_of_keys_lets_the_key_change_over_time() {
        let (old, new) = (ConfigSigningKey::generate(), ConfigSigningKey::generate());
        let list = format!(
            "# keys this image trusts\n{}\n\n{}\n",
            old.verifying_key().to_base64(),
            new.verifying_key().to_base64()
        );
        let trusted = parse_verifying_keys(&list).expect("keys");
        assert_eq!(trusted.len(), 2);
        for key in [&old, &new] {
            assert_eq!(
                verify_config(&trusted, SETTINGS, &key.sign(SETTINGS).to_file_text()),
                Ok(())
            );
        }
        assert_eq!(
            parse_verifying_keys("not a key\n"),
            Err(SignatureError::NotAKey)
        );
    }

    #[test]
    fn keys_survive_being_written_and_read_and_the_secret_is_not_in_debug() {
        let key = ConfigSigningKey::generate();
        let again = ConfigSigningKey::from_seed_base64(&key.seed_base64()).expect("seed");
        assert_eq!(again.verifying_key(), key.verifying_key());
        assert!(!format!("{key:?}").contains(&key.seed_base64()));
        assert_eq!(
            ConfigVerifyingKey::from_base64(&key.verifying_key().to_base64()),
            Ok(key.verifying_key())
        );
        assert_eq!(key.verifying_key().key_id().len(), 16);
        for bad in ["", "AAAA", "not base64!"] {
            assert!(ConfigSigningKey::from_seed_base64(bad).is_err());
            assert!(ConfigVerifyingKey::from_base64(bad).is_err());
        }
    }

    #[test]
    fn a_signature_for_other_bytes_cannot_be_used_without_the_domain() {
        // The signed message starts with a fixed tag, so a plain Ed25519 signature of the settings
        // made by the same key for another purpose does not verify here.
        let key = ConfigSigningKey::generate();
        let plain = key.0.sign(SETTINGS);
        let forged = ConfigSignature {
            key_id: key.verifying_key().key_id(),
            bytes: plain.to_bytes(),
        };
        assert_eq!(
            verify_config(&[key.verifying_key()], SETTINGS, &forged.to_file_text()),
            Err(SignatureError::Mismatch)
        );
    }
}
