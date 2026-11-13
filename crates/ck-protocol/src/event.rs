//! What a stick reports besides heartbeats: a login, a submission.
//!
//! Every event carries the hash of the one before it in the same boot, so the Hub can see that
//! an event is missing or was changed. This crate holds only the data and the check; the hash
//! function is passed in, because hashing belongs to `ck-crypto`.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::ids::StickId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum EventKind {
    Login,
    Submission,
}

/// The `prev_hash` of the first event of a boot.
pub const FIRST_PREV_HASH: &str = "";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub stick_id: StickId,
    /// Changes at every boot. The chain starts again with each one.
    pub boot_id: String,
    /// Starts at 1 each boot and goes up by one.
    pub seq: u64,
    /// The hash of the previous event of this boot, or [`FIRST_PREV_HASH`] for the first.
    pub prev_hash: String,
    /// By the stick's clock. The Hub also records its own receive time.
    pub sent_at: DateTime<Utc>,
    pub kind: EventKind,
    /// What happened. Its shape depends on `kind` and is fixed with the capture rules (B.3, D.2).
    /// Never a password, a cookie or a session id.
    pub payload: Value,
}

impl Event {
    /// The bytes that are hashed and signed. Fields come out in a fixed order and the keys of the
    /// payload are sorted, so two programs get the same bytes for the same event.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("an event always serializes")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChainError {
    #[error("event {at} is for another stick or boot than the one before it")]
    Mixed { at: usize },
    #[error("event {at} has sequence {found}, expected {expected}: an event is missing")]
    Gap {
        at: usize,
        found: u64,
        expected: u64,
    },
    #[error("event {at} does not follow the one before it: that event was changed or replaced")]
    BrokenLink { at: usize },
}

/// Checks that `events`, all from one stick and one boot and in order, form an unbroken chain.
///
/// `start_seq` is the sequence number the first event must have: 1 for a whole boot, higher when
/// the list is a later part and `previous_hash` is the hash of the event just before it.
///
/// # Errors
/// The first problem found, with the position of the event in `events`.
pub fn verify_chain(
    events: &[Event],
    start_seq: u64,
    previous_hash: &str,
    hash: impl Fn(&[u8]) -> String,
) -> Result<(), ChainError> {
    let mut expected_prev = previous_hash.to_owned();
    for (expected_seq, (at, event)) in (start_seq..).zip(events.iter().enumerate()) {
        if let Some(first) = events.first()
            && (event.stick_id != first.stick_id || event.boot_id != first.boot_id)
        {
            return Err(ChainError::Mixed { at });
        }
        if event.seq != expected_seq {
            return Err(ChainError::Gap {
                at,
                found: event.seq,
                expected: expected_seq,
            });
        }
        if event.prev_hash != expected_prev {
            return Err(ChainError::BrokenLink { at });
        }
        expected_prev = hash(&event.canonical_bytes());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    /// A stand-in for SHA-256 that is enough to tell events apart.
    fn hash(bytes: &[u8]) -> String {
        let sum = bytes.iter().fold(0_u64, |acc, b| {
            acc.wrapping_mul(31).wrapping_add(u64::from(*b))
        });
        format!("{sum:016x}")
    }

    fn event(seq: u64, prev_hash: &str, user: &str) -> Event {
        Event {
            stick_id: StickId::parse("03c9f556").expect("id"),
            boot_id: "boot-1".to_owned(),
            seq,
            prev_hash: prev_hash.to_owned(),
            sent_at: "2026-11-12T10:00:00Z".parse().expect("time"),
            kind: EventKind::Login,
            payload: json!({ "username": user, "platform": "kilonova" }),
        }
    }

    /// `count` events, each pointing at the hash of the one before.
    fn chain(count: u64) -> Vec<Event> {
        let mut events: Vec<Event> = Vec::new();
        for seq in 1..=count {
            let prev = events.last().map_or_else(
                || FIRST_PREV_HASH.to_owned(),
                |e| hash(&e.canonical_bytes()),
            );
            events.push(event(seq, &prev, &format!("user{seq}")));
        }
        events
    }

    #[test]
    fn survives_a_round_trip_and_leaves_no_nulls() {
        let original = event(1, FIRST_PREV_HASH, "ana");
        let json = serde_json::to_value(&original).expect("serializes");
        assert_eq!(json["kind"], "login");
        assert_eq!(
            serde_json::from_value::<Event>(json).expect("valid"),
            original
        );
    }

    #[test]
    fn the_bytes_do_not_depend_on_the_order_the_payload_was_written_in() {
        let mut a = event(1, FIRST_PREV_HASH, "ana");
        let mut b = a.clone();
        a.payload = json!({ "username": "ana", "platform": "kilonova" });
        b.payload = json!({ "platform": "kilonova", "username": "ana" });
        assert_eq!(a.canonical_bytes(), b.canonical_bytes());
    }

    #[test]
    fn an_unbroken_chain_passes_and_a_later_part_can_continue_an_earlier_one() {
        let events = chain(4);
        assert_eq!(verify_chain(&events, 1, FIRST_PREV_HASH, hash), Ok(()));
        assert_eq!(verify_chain(&[], 1, FIRST_PREV_HASH, hash), Ok(()));

        let before = hash(&events[1].canonical_bytes());
        assert_eq!(verify_chain(&events[2..], 3, &before, hash), Ok(()));
    }

    #[test]
    fn a_missing_event_is_a_gap() {
        let mut events = chain(4);
        events.remove(1);
        assert_eq!(
            verify_chain(&events, 1, FIRST_PREV_HASH, hash),
            Err(ChainError::Gap {
                at: 1,
                found: 3,
                expected: 2
            })
        );
    }

    #[test]
    fn a_changed_event_breaks_the_link_after_it() {
        let mut events = chain(4);
        events[1].payload = json!({ "username": "someone else", "platform": "kilonova" });
        assert_eq!(
            verify_chain(&events, 1, FIRST_PREV_HASH, hash),
            Err(ChainError::BrokenLink { at: 2 })
        );
    }

    #[test]
    fn a_first_event_that_claims_a_previous_one_and_mixed_boots_are_refused() {
        let events = vec![event(1, "deadbeef", "ana")];
        assert_eq!(
            verify_chain(&events, 1, FIRST_PREV_HASH, hash),
            Err(ChainError::BrokenLink { at: 0 })
        );

        let mut events = chain(3);
        events[2].boot_id = "boot-2".to_owned();
        assert_eq!(
            verify_chain(&events, 1, FIRST_PREV_HASH, hash),
            Err(ChainError::Mixed { at: 2 })
        );
    }
}
