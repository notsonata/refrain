use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistExport {
    pub id: i64,
    pub collection_id: i64,
    pub collection_name: String,
    pub mode: String,
    pub destination: String,
    pub status: String,
    pub entry_count: usize,
    pub created_at: i64,
    pub finished_at: Option<i64>,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistExportPage {
    pub items: Vec<PlaylistExport>,
    pub total: i64,
    pub offset: u32,
    pub limit: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlaylistExportSource {
    pub collection_id: i64,
    pub collection_name: String,
    pub image_url: Option<String>,
    pub entries: Vec<PlaylistExportCandidateEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlaylistExportCandidateEntry {
    pub position: i64,
    pub source_track_id: Option<i64>,
    pub library_track_id: Option<i64>,
    pub local_file_id: Option<i64>,
    pub title: String,
    pub artists: Vec<String>,
    pub duration_ms: Option<i64>,
    pub file_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PlaylistExportSnapshotEntry {
    pub position: i64,
    pub source_track_id: i64,
    pub library_track_id: i64,
    pub local_file_id: i64,
    pub exported_relative_path: String,
}
