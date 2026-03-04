use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandoffSession {
    pub session_id: String,
    pub target: String,
    pub created_at: DateTime<Utc>,
    pub sources: Vec<SourceSummary>,
    pub refs_index_id: String,
    pub refs: Vec<RefItem>,
    pub discovered_commands: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceSummary {
    pub source: String,
    pub pages: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefMetadata {
    pub path: Option<String>,
    pub line_start: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefItem {
    pub ref_id: String,
    pub source: String,
    pub kind: String,
    pub title: Option<String>,
    pub content: String,
    pub metadata: RefMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitResponse {
    pub session_id: String,
    pub target: String,
    pub sources: Vec<SourceSummary>,
    pub refs_index_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RefListItem {
    pub ref_id: String,
    pub kind: String,
    pub title: Option<String>,
    pub byte_len: usize,
    pub preview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListResponse {
    pub session_id: String,
    pub source: String,
    pub page: usize,
    pub per_page: usize,
    pub total_pages: usize,
    pub items: Vec<RefListItem>,
}
