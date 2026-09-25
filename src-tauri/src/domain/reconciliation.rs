use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ReconciliationCounts {
    pub matched: i64,
    pub missing: i64,
    pub needs_review: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyncRun {
    pub id: i64,
    pub trigger: String,
    pub status: String,
    pub phase: Option<String>,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub source_added: i64,
    pub source_removed: i64,
    pub matched: i64,
    pub missing: i64,
    pub needs_review: i64,
    pub acquisition_failed: i64,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyncRunPage {
    pub items: Vec<SyncRun>,
    pub total: usize,
    pub offset: u32,
    pub limit: u32,
}
