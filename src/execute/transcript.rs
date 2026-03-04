use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionTranscript {
    pub session_id: String,
    pub started_at: DateTime<Utc>,
    pub setup: Vec<StepRunRecord>,
    pub checks: Vec<StepRunRecord>,
    pub scenes: Vec<SceneTranscript>,
    pub cleanup: Vec<StepRunRecord>,
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
