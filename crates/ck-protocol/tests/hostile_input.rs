//! Hostile input: text and JSON from outside never panic the parsers or the validator.
//! The number of cases follows `PROPTEST_CASES`.

use ck_protocol::wifi::{is_valid_password, is_valid_ssid};
use ck_protocol::{
    CommandAck, EnrolmentCode, EnrolmentConfig, Heartbeat, HeartbeatReply, Settings, SignedCommand,
    StickId,
};
use proptest::prelude::*;

/// JSON-looking text, so the generator gets past the first character more often than raw noise.
fn jsonish() -> impl Strategy<Value = String> {
    prop::collection::vec(
        prop_oneof![
            Just("{".to_owned()),
            Just("}".to_owned()),
            Just("[".to_owned()),
            Just("]".to_owned()),
            Just(":".to_owned()),
            Just(",".to_owned()),
            Just("null".to_owned()),
            Just("true".to_owned()),
            Just("\"team\"".to_owned()),
            Just("\"wifi\"".to_owned()),
            Just("\"version\"".to_owned()),
            Just("-1".to_owned()),
            Just("18446744073709551616".to_owned()),
            any::<String>(),
        ],
        0..24,
    )
    .prop_map(|parts| parts.concat())
}

proptest! {
    #[test]
    fn arbitrary_bytes_never_panic_the_json_parsers(bytes in prop::collection::vec(any::<u8>(), 0..512)) {
        let _ = serde_json::from_slice::<Settings>(&bytes);
        let _ = serde_json::from_slice::<SignedCommand>(&bytes);
        let _ = serde_json::from_slice::<CommandAck>(&bytes);
        let _ = serde_json::from_slice::<Heartbeat>(&bytes);
        let _ = serde_json::from_slice::<HeartbeatReply>(&bytes);
        let _ = serde_json::from_slice::<EnrolmentConfig>(&bytes);
    }

    #[test]
    fn jsonish_text_never_panics_parsing_or_validation(text in jsonish()) {
        if let Ok(settings) = serde_json::from_str::<Settings>(&text) {
            let _ = settings.validate();
        }
        let _ = serde_json::from_str::<SignedCommand>(&text);
        let _ = serde_json::from_str::<Heartbeat>(&text);
    }

    #[test]
    fn hostile_settings_fields_never_panic_the_validator(
        team in any::<String>(),
        site in any::<String>(),
        pattern in any::<String>(),
        label in any::<String>(),
    ) {
        let mut settings = Settings {
            team,
            site,
            ..Settings::default()
        };
        settings.branding.computer_name_pattern = pattern;
        settings.branding.usb_label = label;
        let _ = settings.validate();
    }

    #[test]
    fn ids_and_codes_never_panic_and_accepted_ones_round_trip(text in any::<String>()) {
        if let Ok(id) = StickId::parse(&text) {
            prop_assert_eq!(StickId::parse(id.as_str()).unwrap(), id);
        }
        let _ = EnrolmentCode::parse(&text);
        let _ = is_valid_ssid(&text);
        let _ = is_valid_password(&text);
    }
}
