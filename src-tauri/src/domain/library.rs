use serde::{Deserialize, Serialize};

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryTrack {
    pub id: i64,
    pub canonical_source_track_id: Option<i64>,
    pub title: String,
    pub normalized_title: String,
    pub artists: Vec<String>,
    pub normalized_artists: String,
    pub album: Option<String>,
    pub normalized_album: Option<String>,
    pub isrc: Option<String>,
    pub duration_ms: Option<i64>,
    pub disc_number: Option<i64>,
    pub track_number: Option<i64>,
    pub release_year: Option<i64>,
    pub explicit: Option<bool>,
    pub version_kind: Option<String>,
    pub version_detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalFile {
    pub id: i64,
    pub library_track_id: Option<i64>,
    pub path: String,
    pub ownership: String,
    pub is_preferred: bool,
    pub state: String,
    pub format: Option<String>,
    pub file_size: i64,
    pub modified_at: i64,
    pub duration_ms: Option<i64>,
    pub bitrate: Option<i64>,
    pub sample_rate: Option<i64>,
    pub channels: Option<i64>,
    pub content_hash: Option<String>,
    pub tag_title: Option<String>,
    pub tag_artists: Vec<String>,
    pub tag_album: Option<String>,
    pub tag_isrc: Option<String>,
    pub scan_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalFilePage {
    pub items: Vec<LocalFile>,
    pub total: usize,
    pub offset: u32,
    pub limit: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalLibraryOverview {
    pub total: usize,
    pub present: usize,
    pub missing: usize,
    pub invalid: usize,
}
