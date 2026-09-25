use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MatchTrackDescriptor {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MatchOutcome {
    Automatic,
    Review,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MatchRelationship {
    Same,
    Different,
    Uncertain,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchScoreBreakdown {
    pub title: i64,
    pub artists: i64,
    pub duration: i64,
    pub album: i64,
    pub track_disc: i64,
    pub total: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchCandidateEvidence {
    pub library_track_id: i64,
    pub relationship: MatchRelationship,
    pub score: MatchScoreBreakdown,
    pub exact_isrc: bool,
    pub duration_difference_ms: Option<i64>,
    pub warnings: Vec<String>,
    pub incompatibilities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MatchResult {
    pub source_track_id: i64,
    pub outcome: MatchOutcome,
    pub selected_library_track_id: Option<i64>,
    pub method: Option<String>,
    pub confidence: Option<i64>,
    pub candidates: Vec<MatchCandidateEvidence>,
    pub rejected_library_track_ids: Vec<i64>,
}
