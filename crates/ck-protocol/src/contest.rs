//! Which contest the stick is for.

use serde::{Deserialize, Serialize};

use crate::time::LocalDateTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    #[default]
    Kilonova,
    Codeforces,
    Vjudge,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Contest {
    pub name: String,
    pub platform: Platform,
    pub contest_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<LocalDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<LocalDateTime>,
    pub practice: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_uses_lowercase_names() {
        assert_eq!(
            serde_json::to_string(&Platform::Vjudge).expect("serializes"),
            "\"vjudge\""
        );
        assert_eq!(
            serde_json::from_str::<Platform>("\"codeforces\"").expect("valid"),
            Platform::Codeforces
        );
        assert!(serde_json::from_str::<Platform>("\"Kilonova\"").is_err());
    }

    #[test]
    fn missing_fields_take_defaults() {
        let contest: Contest = serde_json::from_str(r#"{"name":"RCPC"}"#).expect("valid json");
        assert_eq!(contest.platform, Platform::Kilonova);
        assert_eq!(contest.contest_id, "");
        assert!(contest.start.is_none());
        assert!(!contest.practice);
    }

    #[test]
    fn missing_times_are_left_out_when_written() {
        let json = serde_json::to_string(&Contest::default()).expect("serializes");
        assert!(!json.contains("start") && !json.contains("end") && !json.contains("null"));
    }
}
