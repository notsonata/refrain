use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalPlaylistSummary {
    pub id: i64,
    pub name: String,
    pub source_collection_id: Option<i64>,
    pub image_url: Option<String>,
    pub entry_count: usize,
    pub m3u_path: Option<String>,
    pub m3u_managed: bool,
    pub last_synced_at: Option<i64>,
    pub sync_error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalPlaylistEntry {
    pub id: i64,
    pub position: usize,
    pub library_track_id: i64,
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub duration_ms: Option<i64>,
    pub file_path: Option<String>,
    pub artwork_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalPlaylistDetail {
    pub id: i64,
    pub name: String,
    pub source_collection_id: Option<i64>,
    pub image_url: Option<String>,
    pub m3u_path: Option<String>,
    pub m3u_managed: bool,
    pub last_synced_at: Option<i64>,
    pub sync_error: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub entries: Vec<LocalPlaylistEntry>,
}
