#[allow(dead_code)]
pub(crate) mod acquisition;
pub(crate) mod issues;
pub(crate) mod library;
pub(crate) mod matching;
mod playlist_exports;
mod playlists;
pub(crate) mod reconciliation;
mod settings;
mod source;

pub use acquisition::{
    AcquisitionCandidate, AcquisitionJob, AcquisitionJobPage, AcquisitionRequest, ProviderHealth,
    ProviderJob, ProviderJobStatus, StagingItem, StagingPage, TrackQuery,
};
pub use issues::{
    IssueCounts, IssueKind, IssuePage, IssueRow, MatchReview, MatchReviewCandidate,
    MatchReviewTrack,
};
pub use library::{
    LibraryTrackFileSummary, LibraryTrackPage, LibraryTrackRow, LocalFile, LocalFilePage,
    LocalLibraryOverview, SpotifyMembership,
};
pub(crate) use matching::MatchTrackDescriptor;
pub use matching::{
    MatchCandidateEvidence, MatchOutcome, MatchRelationship, MatchResult, MatchScoreBreakdown,
};
pub use playlist_exports::{PlaylistExport, PlaylistExportPage};
pub(crate) use playlist_exports::{
    PlaylistExportCandidateEntry, PlaylistExportSnapshotEntry, PlaylistExportSource,
};
pub use playlists::{LocalPlaylistDetail, LocalPlaylistEntry, LocalPlaylistSummary};
pub(crate) use reconciliation::ReconciliationCounts;
pub use reconciliation::{SyncRun, SyncRunPage};
pub use settings::AppSettings;
pub use source::{
    CollectionEntry, SourceAccount, SourceAccountOverview, SourceAlbumMetadata, SourceCollection,
    SourceCollectionEntryView, SourceCollectionItem, SourceCollectionListPage,
    SourceCollectionPage, SourceCollectionSummary, SourceTrack, SourceTrackView,
    SpotifySourceOverview,
};
