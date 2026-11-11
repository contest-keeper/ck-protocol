//! The data types shared by Keeper OS, Keeper Forge and Keeper Hub.
//!
//! The admin tool writes a [`StickConfig`] as `ck/config.json` on the stick's EFI partition. The
//! agent applies it at boot and sends [`Heartbeat`]s to the Hub. This crate holds only data types
//! and pure functions: no files, no network and no clock.
//!
//! Field names are `snake_case`. A value that is missing is left out when writing, never `null`, and
//! unknown fields are ignored when reading, so a newer writer does not break an older reader.

pub mod api;
pub mod branding;
pub mod contest;
pub mod heartbeat;
pub mod ids;
pub mod network;
pub mod recording;
pub mod settings;
pub mod time;
pub mod validation;

pub use api::Api;
pub use branding::{Bookmark, Branding, render_computer_name};
pub use contest::{Contest, Platform};
pub use heartbeat::{Heartbeat, RecorderState, RecorderStatus};
pub use ids::{InvalidStickId, StickId, Token};
pub use network::Network;
pub use recording::{Recording, RecordingMode};
pub use settings::{ESP_CONFIG_DIR, ESP_CONFIG_FILE, SCHEMA, Settings, StickConfig};
pub use time::{LocalDateTime, ParseLocalDateTimeError};
pub use validation::Problem;

/// HTTP header that carries the stick's id.
pub const STICK_HEADER: &str = "X-CK-Stick";

/// HTTP header that carries the signature of the request body.
pub const SIGNATURE_HEADER: &str = "X-CK-Signature";

/// Path the stick sends heartbeats to, below the Hub's address.
pub const HEARTBEAT_PATH: &str = "/api/v1/heartbeat";
