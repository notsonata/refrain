use serde::{Deserialize, Serialize};

use super::{LocalFile, MatchCandidateEvidence, MatchOutcome};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IssueKind {
    MatchReview,
    MissingLocalFile,
    LocalOnlyTrack,
    InaccessibleCollection,
    InvalidLocalFile,
    AcquisitionFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueRow {
    pub id: String,
    pub kind: IssueKind,
    pub title: String,
    pub subtitle: Option<String>,
    pub detail: Option<String>,
    pub source_track_id: Option<i64>,
    pub library_track_id: Option<i64>,
    pub local_file_id: Option<i64>,
    pub collection_id: Option<i64>,
    pub candidate_count: Option<usize>,
    pub confidence: Option<i64>,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssueCounts {
    pub match_review: usize,
    pub missing_local_file: usize,
    pub local_only_track: usize,
    pub inaccessible_collection: usize,
    pub invalid_local_file: usize,
    pub acquisition_failed: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IssuePage {
    pub items: Vec<IssueRow>,
    pub total: usize,
    pub offset: u32,
    pub limit: u32,
    pub counts: IssueCounts,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchReviewTrack {
    pub id: i64,
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub isrc: Option<String>,
    pub duration_ms: Option<i64>,
    pub disc_number: Option<i64>,
    pub track_number: Option<i64>,
    pub explicit: Option<bool>,
    pub version_kind: Option<String>,
    pub version_detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchReviewCandidate {
    pub track: MatchReviewTrack,
    pub evidence: MatchCandidateEvidence,
    pub files: Vec<LocalFile>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchReview {
    pub source: MatchReviewTrack,
    pub outcome: MatchOutcome,
    pub selected_library_track_id: Option<i64>,
    pub method: Option<String>,
    pub confidence: Option<i64>,
    pub rejected_library_track_ids: Vec<i64>,
    pub candidates: Vec<MatchReviewCandidate>,
}
