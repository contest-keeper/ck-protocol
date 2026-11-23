//! Hostile input: keys, signature files and signatures from outside are refused, never a panic,
//! and a signature that is not the real one never verifies. The number of cases follows
//! `PROPTEST_CASES`.

use ck_crypto::{
    ConfigSignature, ConfigSigningKey, ConfigVerifyingKey, parse_verifying_keys, sign, verify,
    verify_config,
};
use ck_protocol::Token;
use proptest::prelude::*;

proptest! {
    #[test]
    fn arbitrary_text_never_panics_the_key_and_signature_parsers(text in any::<String>()) {
        let _ = ConfigSigningKey::from_seed_base64(&text);
        let _ = ConfigVerifyingKey::from_base64(&text);
        let _ = parse_verifying_keys(&text);
        let _ = ConfigSignature::parse(&text);
    }

    #[test]
    fn a_made_up_signature_file_never_verifies(
        config in prop::collection::vec(any::<u8>(), 0..256),
        file in any::<String>(),
    ) {
        let key = ConfigSigningKey::generate().verifying_key();
        prop_assert!(verify_config(&[key], &config, &file).is_err());
    }

    #[test]
    fn a_changed_config_never_verifies(
        config in prop::collection::vec(any::<u8>(), 1..256),
        flip in any::<prop::sample::Index>(),
    ) {
        let signing = ConfigSigningKey::generate();
        let file = signing.sign(&config).to_file_text();
        let mut changed = config.clone();
        let at = flip.index(changed.len());
        changed[at] ^= 1;
        prop_assert!(verify_config(&[signing.verifying_key()], &config, &file).is_ok());
        prop_assert!(verify_config(&[signing.verifying_key()], &changed, &file).is_err());
    }

    #[test]
    fn a_made_up_hmac_never_verifies(
        body in prop::collection::vec(any::<u8>(), 0..256),
        signature in any::<String>(),
    ) {
        let token = Token::new("a-fixed-test-secret");
        prop_assume!(signature != sign(&token, &body));
        prop_assert!(!verify(&token, &body, &signature));
    }
}
