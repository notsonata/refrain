#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceAccount {
    pub provider: String,
    pub provider_account_id: String,
    pub display_name: Option<String>,
    pub client_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceCollection {
    pub source_account_id: i64,
    pub provider_collection_id: String,
    pub kind: String,
    pub name: String,
    pub snapshot_id: Option<String>,
    pub owner_provider_id: Option<String>,
    pub is_accessible: bool,
    pub access_issue: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceTrack {
    pub provider: String,
    pub provider_track_id: String,
    pub uri: Option<String>,
    pub isrc: Option<String>,
    pub title: String,
    pub normalized_title: String,
    pub artists_json: String,
    pub normalized_artists: String,
    pub album: Option<String>,
    pub normalized_album: Option<String>,
    pub duration_ms: Option<i64>,
    pub disc_number: Option<i64>,
    pub track_number: Option<i64>,
    pub release_year: Option<i64>,
    pub explicit: Option<bool>,
    pub version_kind: Option<String>,
    pub version_detail: Option<String>,
    pub image_url: Option<String>,
    pub external_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionEntry {
    pub position: i64,
    pub source_track_id: Option<i64>,
    pub provider_item_uri: Option<String>,
    pub item_type: String,
    pub added_at: Option<i64>,
    pub unavailable_reason: Option<String>,
}
