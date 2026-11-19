//! What the Hub can tell a stick to do, and what the stick says back.
//!
//! The design, the checks and the replay rule are in `docs/commands.md`. This module holds the data
//! and the checks that need no secret; signing is in `ck-crypto`.

use serde::{Deserialize, Serialize};

use crate::ids::StickId;
use crate::time::Timestamp;

/// A command may be valid for at most this long.
pub const MAX_LIFETIME_S: i64 = 3600;

/// How far ahead of the stick's clock `issued_at` may be.
pub const CLOCK_SKEW_S: i64 = 300;

pub const MAX_MESSAGE_LENGTH: usize = 500;

const MAX_ID_LENGTH: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum Action {
    /// Shown as a desktop notification.
    Message {
        text: String,
    },
    LockScreen,
    UnlockScreen,
    Shutdown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Command {
    /// Unique per command; the stick runs each id once.
    pub id: String,
    pub stick_id: StickId,
    pub issued_at: Timestamp,
    pub expires_at: Timestamp,
    pub action: Action,
}

impl Command {
    /// The bytes that are signed: the compact JSON of the command, fields in a fixed order.
    #[must_use]
    pub fn canonical_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("a command always serializes")
    }

    /// Everything that can be checked without the secret. `now` is the stick's clock.
    ///
    /// # Errors
    /// The first reason the stick must not run this command.
    pub fn check(&self, stick: &StickId, now: Timestamp) -> Result<(), Refusal> {
        if &self.stick_id != stick {
            return Err(Refusal::WrongStick);
        }
        let issued = self.issued_at.as_offset_date_time();
        let expires = self.expires_at.as_offset_date_time();
        let now = now.as_offset_date_time();
        if expires <= now {
            return Err(Refusal::Expired);
        }
        if issued > now + time::Duration::seconds(CLOCK_SKEW_S) {
            return Err(Refusal::NotYetValid);
        }
        if expires <= issued || expires - issued > time::Duration::seconds(MAX_LIFETIME_S) {
            return Err(Refusal::BadLifetime);
        }
        if !self.is_well_formed() {
            return Err(Refusal::Malformed);
        }
        Ok(())
    }

    fn is_well_formed(&self) -> bool {
        let id_ok = (1..=MAX_ID_LENGTH).contains(&self.id.len())
            && self
                .id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-');
        let action_ok = match &self.action {
            Action::Message { text } => {
                let length = text.chars().count();
                (1..=MAX_MESSAGE_LENGTH).contains(&length)
                    && !text.chars().any(|c| c.is_control() && c != '\n')
            }
            _ => true,
        };
        id_ok && action_ok
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedCommand {
    pub command: Command,
    /// Lowercase hex HMAC-SHA256 of `command.canonical_bytes()`.
    pub signature: String,
}

/// Why a stick did not run a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum Refusal {
    #[error("the signature is not valid")]
    BadSignature,
    #[error("the command is for another stick")]
    WrongStick,
    #[error("the command has expired")]
    Expired,
    #[error("the command is dated in the future")]
    NotYetValid,
    #[error("the command is valid for longer than an hour, or ends before it starts")]
    BadLifetime,
    #[error("the command was already run")]
    Replayed,
    #[error("the command id or text is not valid")]
    Malformed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AckResult {
    Ok,
    Refused,
    Failed,
}

/// The payload of an event of kind `command_ack`. It never repeats the message text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandAck {
    pub command_id: String,
    pub result: AckResult,
    /// Set when `result` is `refused`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refusal: Option<Refusal>,
}

/// The Hub's answer to a heartbeat.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct HeartbeatReply {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub commands: Vec<SignedCommand>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> Timestamp {
        text.parse().expect("time")
    }

    fn stick() -> StickId {
        StickId::parse("03c9f556").expect("id")
    }

    fn command(action: Action) -> Command {
        Command {
            id: "c-1".to_owned(),
            stick_id: stick(),
            issued_at: at("2026-11-18T10:00:00Z"),
            expires_at: at("2026-11-18T10:05:00Z"),
            action,
        }
    }

    #[test]
    fn actions_are_written_with_a_type_tag() {
        let message = command(Action::Message { text: "hi".into() });
        let json = serde_json::to_value(message).expect("json");
        assert_eq!(
            json["action"],
            serde_json::json!({"type": "message", "text": "hi"})
        );
        let json = serde_json::to_value(command(Action::LockScreen)).expect("json");
        assert_eq!(json["action"], serde_json::json!({"type": "lock_screen"}));
        let json = serde_json::to_value(command(Action::Shutdown)).expect("json");
        assert_eq!(json["action"], serde_json::json!({"type": "shutdown"}));
    }

    #[test]
    fn an_unlock_command_round_trips_with_the_same_type_tag() {
        let action = serde_json::json!({"type": "unlock_screen"});
        let unlock: Action = serde_json::from_value(action.clone()).expect("known action");
        let command = command(unlock);
        assert_eq!(command.check(&stick(), at("2026-11-18T10:01:00Z")), Ok(()));
        assert_eq!(
            serde_json::to_value(command).expect("json")["action"],
            action
        );
    }

    #[test]
    fn a_fresh_command_for_this_stick_passes() {
        let c = command(Action::Shutdown);
        assert_eq!(c.check(&stick(), at("2026-11-18T10:01:00Z")), Ok(()));
        // The stick's clock a little behind the Hub's is fine.
        assert_eq!(c.check(&stick(), at("2026-11-18T09:57:00Z")), Ok(()));
    }

    #[test]
    fn refuses_other_sticks_expired_and_future_commands() {
        let c = command(Action::LockScreen);
        let other = StickId::parse("other").expect("id");
        let now = at("2026-11-18T10:01:00Z");
        assert_eq!(c.check(&other, now), Err(Refusal::WrongStick));
        assert_eq!(
            c.check(&stick(), at("2026-11-18T10:05:00Z")),
            Err(Refusal::Expired)
        );
        assert_eq!(
            c.check(&stick(), at("2026-11-18T09:50:00Z")),
            Err(Refusal::NotYetValid)
        );
    }

    #[test]
    fn refuses_a_lifetime_over_an_hour_and_bad_text() {
        let now = at("2026-11-18T10:01:00Z");
        let mut c = command(Action::LockScreen);
        c.expires_at = at("2026-11-18T11:00:01Z");
        assert_eq!(c.check(&stick(), now), Err(Refusal::BadLifetime));

        let long = command(Action::Message {
            text: "x".repeat(501),
        });
        assert_eq!(long.check(&stick(), now), Err(Refusal::Malformed));
        let empty = command(Action::Message {
            text: String::new(),
        });
        assert_eq!(empty.check(&stick(), now), Err(Refusal::Malformed));
        let mut bad_id = command(Action::Shutdown);
        bad_id.id = "a b".to_owned();
        assert_eq!(bad_id.check(&stick(), now), Err(Refusal::Malformed));
    }

    #[test]
    fn an_empty_reply_has_no_commands_and_an_ack_leaves_out_nulls() {
        assert_eq!(
            serde_json::to_string(&HeartbeatReply::default()).expect("json"),
            "{}"
        );
        let reply: HeartbeatReply = serde_json::from_str("{}").expect("valid");
        assert_eq!(reply.commands, []);
        let ack = CommandAck {
            command_id: "c-1".into(),
            result: AckResult::Ok,
            refusal: None,
        };
        assert_eq!(
            serde_json::to_string(&ack).expect("json"),
            r#"{"command_id":"c-1","result":"ok"}"#
        );
    }
}
