use serde::{Deserialize, Serialize};

use crate::branding::BrandingConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DemoScript {
    pub version: String,
    #[serde(default)]
    pub setup: Vec<ScriptStep>,
    #[serde(default)]
    pub scenes: Vec<ScriptScene>,
    #[serde(default)]
    pub checks: Vec<ScriptStep>,
    #[serde(default)]
    pub cleanup: Vec<ScriptStep>,
    #[serde(default)]
    pub redactions: Vec<RedactRule>,
    pub audio: Option<AudioConfig>,
    #[serde(default)]
    pub branding: Option<BrandingConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptScene {
    pub id: String,
    pub title: String,
    pub steps: Vec<ScriptStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptStep {
    pub id: String,
    pub run: String,
    pub expect: Option<ExpectCondition>,
    pub timeout_ms: Option<u64>,
    pub source_refs: Vec<String>,
    #[serde(default)]
    pub manual_step: bool,
    pub manual_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExpectCondition {
    pub contains: Option<String>,
    pub regex: Option<String>,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RedactRule {
    pub pattern: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioConfig {
    #[serde(default)]
    pub typing: bool,
    pub music_path: Option<String>,
}
