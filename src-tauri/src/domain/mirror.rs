use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MirrorTarget {
    pub id: i64,
    pub path: String,
    pub identity: String,
    pub created_at: i64,
    pub updated_at: i64,
    pub last_run: Option<MirrorRun>,
    pub available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MirrorRun {
    pub id: i64,
    pub target_id: i64,
    pub status: String,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub copied: i64,
    pub unchanged: i64,
    pub removed: i64,
    pub error: Option<String>,
}
