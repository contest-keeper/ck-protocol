//! The data types shared by Keeper OS, Keeper Forge and Keeper Hub.
//!
//! The admin tool writes a [`StickConfig`] as `config.json` on the stick's `CKCONFIG` partition. The
//! agent applies it at boot and sends [`Heartbeat`]s to the Hub. This crate holds only data types
//! and pure functions: no files, no network and no clock.
//!
//! Field names are `snake_case`. A value that is missing is left out when writing, never `null`, and
//! unknown fields are ignored when reading, so a newer writer does not break an older reader.

pub mod api;
pub mod branding;
pub mod command;
pub mod contest;
pub mod event;
pub mod heartbeat;
pub mod ids;
pub mod network;
pub mod recording;
pub mod settings;
pub mod time;
pub mod validation;
pub mod wifi;

pub use self::time::{LocalDateTime, ParseLocalDateTimeError, ParseTimestampError, Timestamp};
pub use api::Api;
pub use branding::{Bookmark, Branding, render_computer_name};
pub use command::{AckResult, Action, Command, CommandAck, HeartbeatReply, Refusal, SignedCommand};
pub use contest::{Contest, Platform};
pub use event::{ChainError, Event, EventKind, FIRST_PREV_HASH, verify_chain};
pub use heartbeat::{ConfigTrust, Heartbeat, RecorderState, RecorderStatus};
pub use ids::{InvalidStickId, StickId, Token};
pub use network::Network;
pub use recording::{Recording, RecordingMode, UploadMode};
pub use settings::{
    CONFIG_FILE, CONFIG_MOUNT_POINT, CONFIG_PARTITION_LABEL, CONFIG_PUBLIC_KEYS_PATH,
    ENROLMENT_SCHEMA, ESP_CONFIG_DIR, ESP_CONFIG_FILE, EnrolmentCode, EnrolmentConfig,
    InvalidEnrolmentCode, LEGACY_SCHEMA, SCHEMA, Settings, StickConfig,
};
pub use validation::Problem;
pub use wifi::{Wifi, WifiPassword};

pub const STICK_HEADER: &str = "X-CK-Stick";
pub const SIGNATURE_HEADER: &str = "X-CK-Signature";
pub const HEARTBEAT_PATH: &str = "/api/v1/heartbeat";
