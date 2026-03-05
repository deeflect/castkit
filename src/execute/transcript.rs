use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::script::{ArtifactEnter, ArtifactPosition, DemoMode};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionTranscript {
    pub session_id: String,
    pub started_at: DateTime<Utc>,
    pub mode: DemoMode,
    pub setup: Vec<StepRunRecord>,
    pub checks: Vec<StepRunRecord>,
    pub scenes: Vec<SceneTranscript>,
    pub cleanup: Vec<StepRunRecord>,
    #[serde(default)]
    pub overlay_events: Vec<OverlayEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SceneTranscript {
    pub id: String,
    pub title: String,
    pub steps: Vec<StepRunRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepRunRecord {
    pub id: String,
    pub run: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub duration_ms: u128,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OverlayArtifactType {
    Image,
    ResultCard,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlayResultItem {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverlayEvent {
    pub t_ms: u64,
    pub step_id: String,
    pub artifact_type: OverlayArtifactType,
    pub title: Option<String>,
    pub image_path: Option<String>,
    #[serde(default)]
    pub result_items: Vec<OverlayResultItem>,
    pub position: ArtifactPosition,
    pub show_ms: u64,
    pub enter: ArtifactEnter,
}
