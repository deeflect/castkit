use serde::{Deserialize, Serialize};

use crate::branding::BrandingConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DemoScript {
    pub version: String,
    #[serde(default = "default_mode")]
    pub mode: DemoMode,
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
    #[serde(default)]
    pub web: Option<WebConfig>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DemoMode {
    Terminal,
    Web,
}

fn default_mode() -> DemoMode {
    DemoMode::Terminal
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
    #[serde(default)]
    pub artifacts: Vec<StepArtifact>,
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactPosition {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Center,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactEnter {
    Fade,
    Slide,
    Scale,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactDisplay {
    pub title: Option<String>,
    pub position: Option<ArtifactPosition>,
    pub show_ms: Option<u64>,
    pub enter: Option<ArtifactEnter>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ChartType {
    Line,
    Bar,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StepArtifact {
    Image(ImageArtifact),
    WebSnapshot(WebSnapshotArtifact),
    ResultCard(ResultCardArtifact),
    Chart(ChartArtifact),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImageArtifact {
    pub path: String,
    #[serde(flatten)]
    pub display: ArtifactDisplay,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebSnapshotArtifact {
    pub url: String,
    pub path: Option<String>,
    pub wait_for_selector: Option<String>,
    pub clip_selector: Option<String>,
    #[serde(flatten)]
    pub display: ArtifactDisplay,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResultCardArtifact {
    #[serde(default)]
    pub items: Vec<ResultCardItem>,
    #[serde(flatten)]
    pub display: ArtifactDisplay,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResultCardItem {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChartArtifact {
    pub chart_type: ChartType,
    pub data_path: String,
    #[serde(flatten)]
    pub display: ArtifactDisplay,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebConfig {
    pub base_url: Option<String>,
    pub viewport: Option<WebViewport>,
    #[serde(default)]
    pub actions: Vec<WebAction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebViewport {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WebActionType {
    Goto,
    Click,
    Type,
    Press,
    WaitForSelector,
    WaitMs,
    AssertText,
    Screenshot,
    ScrollTo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WebAction {
    pub id: String,
    #[serde(rename = "type")]
    pub action_type: WebActionType,
    #[serde(default)]
    pub source_refs: Vec<String>,
    pub url: Option<String>,
    pub selector: Option<String>,
    pub text: Option<String>,
    pub key: Option<String>,
    pub wait_ms: Option<u64>,
    pub path: Option<String>,
}
