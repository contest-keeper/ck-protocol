//! Makes the key the Hub signs stick settings with.
//!
//! Run it once: `cargo run -p ck-crypto --example config_key`. The secret goes into the Hub's
//! secret store as `CK_CONFIG_SIGNING_KEY` and nowhere else, never into git. The public key goes
//! into the images that should only accept settings the Hub signed.

#![allow(clippy::print_stdout)]

use ck_crypto::ConfigSigningKey;

fn main() {
    let key = ConfigSigningKey::generate();
    let public = key.verifying_key();
    println!(
        "secret (the Hub's CK_CONFIG_SIGNING_KEY): {}",
        key.seed_base64()
    );
    println!("public (for the image): {}", public.to_base64());
    println!("key id: {}", public.key_id());
}
