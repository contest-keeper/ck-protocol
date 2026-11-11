//! Stick secrets and the signatures that prove a report came from a stick.
//!
//! Signing sits behind the [`Signer`] and [`Verifier`] traits, so Ed25519 can replace HMAC later
//! without changes in the programs that use them.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use ck_protocol::Token;
use hmac::{Hmac, KeyInit, Mac};
use sha2::{Digest, Sha256};

const TOKEN_BYTES: usize = 32;

/// Makes the secret for a new stick: 32 random bytes as URL-safe base64 without padding.
#[must_use]
pub fn generate_stick_token() -> Token {
    let mut bytes = [0_u8; TOKEN_BYTES];
    rand::fill(&mut bytes);
    Token::new(URL_SAFE_NO_PAD.encode(bytes))
}

/// HMAC-SHA256 of `body` with the stick's secret, as lowercase hex.
#[must_use]
pub fn sign(token: &Token, body: &[u8]) -> String {
    hex::encode(mac(token, body).finalize().into_bytes())
}

/// Checks a signature made by [`sign`]. Malformed hex is simply not valid.
#[must_use]
pub fn verify(token: &Token, body: &[u8], signature: &str) -> bool {
    let Ok(signature) = hex::decode(signature) else {
        return false;
    };
    mac(token, body).verify_slice(&signature).is_ok()
}

/// SHA-256 of `bytes` as lowercase hex, for settings hashes and code fingerprints.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn mac(token: &Token, body: &[u8]) -> Hmac<Sha256> {
    // HMAC accepts a key of any length, so this cannot fail.
    let mut mac = Hmac::<Sha256>::new_from_slice(token.expose().as_bytes())
        .expect("HMAC accepts any key length");
    mac.update(body);
    mac
}

/// Signs a request body.
pub trait Signer {
    /// The signature of `body`, as text for the `X-CK-Signature` header.
    fn sign(&self, body: &[u8]) -> String;
}

/// Checks the signature of a request body.
pub trait Verifier {
    /// Whether `signature` is valid for `body`.
    fn verify(&self, body: &[u8], signature: &str) -> bool;
}

/// HMAC-SHA256 with one stick's secret. Both sides hold the same secret.
pub struct HmacSha256(Token);

impl HmacSha256 {
    /// Uses `token` as the shared secret.
    #[must_use]
    pub fn new(token: Token) -> Self {
        Self(token)
    }
}

impl Signer for HmacSha256 {
    fn sign(&self, body: &[u8]) -> String {
        sign(&self.0, body)
    }
}

impl Verifier for HmacSha256 {
    fn verify(&self, body: &[u8], signature: &str) -> bool {
        verify(&self.0, body, signature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_the_rfc_4231_vectors() {
        let cases = [
            (
                "\u{b}".repeat(20),
                "Hi There",
                "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7",
            ),
            (
                "Jefe".to_owned(),
                "what do ya want for nothing?",
                "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843",
            ),
        ];
        for (key, data, expected) in cases {
            assert_eq!(sign(&Token::new(key), data.as_bytes()), expected);
        }
    }

    #[test]
    fn rejects_a_changed_body_a_wrong_secret_and_bad_hex() {
        let token = Token::new("secret");
        let signature = sign(&token, b"body");
        assert!(verify(&token, b"body", &signature));
        assert!(!verify(&token, b"body2", &signature));
        assert!(!verify(&Token::new("other"), b"body", &signature));
        assert!(!verify(&token, b"body", "not hex"));
    }

    #[test]
    fn a_signer_and_a_verifier_with_the_same_secret_agree() {
        let stick = HmacSha256::new(Token::new("secret"));
        let signature = stick.sign(b"body");
        assert!(stick.verify(b"body", &signature));
    }

    #[test]
    fn makes_a_different_secret_every_time() {
        let first = generate_stick_token();
        assert_eq!(first.expose().len(), 43);
        assert_ne!(first, generate_stick_token());
    }

    #[test]
    fn hashes_with_sha256() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
