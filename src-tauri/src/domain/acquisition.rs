use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackQuery {
    pub library_track_id: i64,
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub duration_ms: Option<i64>,
    pub isrc: Option<String>,
    pub version_kind: Option<String>,
    pub version_detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcquisitionCandidate {
    #[serde(default)]
    pub provider: Option<String>,
    pub provider_token: String,
    pub source: Option<String>,
    pub file_name: Option<String>,
    pub title: Option<String>,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub duration_ms: Option<i64>,
    pub format: Option<String>,
    pub size_bytes: Option<i64>,
    #[serde(default)]
    pub bitrate_kbps: Option<i64>,
    #[serde(default)]
    pub sample_rate_hz: Option<i64>,
    #[serde(default)]
    pub bit_depth: Option<i64>,
    #[serde(default)]
    pub confidence: Option<u8>,
    #[serde(default)]
    pub isrc: Option<String>,
    #[serde(default)]
    pub recording_id: Option<String>,
    #[serde(default)]
    pub release_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcquisitionRequest {
    pub library_track_id: i64,
    pub query: TrackQuery,
    pub candidate: AcquisitionCandidate,
    pub staging_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderJob {
    pub provider_job_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderJobStatus {
    Pending,
    Running,
    Succeeded {
        output_paths: Vec<String>,
    },
    Failed {
        code: String,
        message: String,
        retryable: bool,
    },
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderHealth {
    pub available: bool,
    pub version: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcquisitionJob {
    pub id: i64,
    pub library_track_id: i64,
    pub track_title: String,
    pub track_artists: Vec<String>,
    pub provider: String,
    pub provider_job_id: Option<String>,
    pub status: String,
    pub stage: Option<String>,
    pub attempt: i64,
    pub candidate: Option<AcquisitionCandidate>,
    pub candidates: Vec<AcquisitionCandidate>,
    pub staging_path: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub created_at: i64,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcquisitionJobPage {
    pub items: Vec<AcquisitionJob>,
    pub total: usize,
    pub offset: u32,
    pub limit: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StagingItem {
    pub library_track_id: i64,
    pub job_id: Option<i64>,
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub duration_ms: Option<i64>,
    pub image_url: Option<String>,
    pub provider: Option<String>,
    pub provider_job_id: Option<String>,
    pub stage: String,
    pub job_status: Option<String>,
    pub attempt: i64,
    pub candidate: Option<AcquisitionCandidate>,
    pub candidates: Vec<AcquisitionCandidate>,
    pub staging_path: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub bytes_transferred: Option<i64>,
    pub total_bytes: Option<i64>,
    pub created_at: Option<i64>,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
    pub updated_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StagingPage {
    pub items: Vec<StagingItem>,
    pub total: usize,
    pub offset: u32,
    pub limit: u32,
}
