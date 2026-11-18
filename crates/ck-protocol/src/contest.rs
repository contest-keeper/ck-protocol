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
