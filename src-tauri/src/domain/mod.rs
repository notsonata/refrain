#[allow(dead_code)]
pub(crate) mod acquisition;
pub(crate) mod issues;
pub(crate) mod library;
pub(crate) mod matching;
pub(crate) mod reconciliation;
mod settings;
mod source;

pub use acquisition::{
    AcquisitionCandidate, AcquisitionJob, AcquisitionJobPage, AcquisitionRequest, ProviderHealth,
    ProviderJob, ProviderJobStatus, TrackQuery,
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
pub(crate) use reconciliation::ReconciliationCounts;
pub use reconciliation::{SyncRun, SyncRunPage};
pub use settings::AppSettings;
pub use source::{
    CollectionEntry, SourceAccount, SourceAccountOverview, SourceCollection,
    SourceCollectionEntryView, SourceCollectionItem, SourceCollectionListPage,
    SourceCollectionPage, SourceCollectionSummary, SourceTrack, SourceTrackView,
    SpotifySourceOverview,
};
