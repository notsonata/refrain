pub(crate) mod library;
pub(crate) mod matching;
mod settings;
mod source;

pub use library::{LocalFile, LocalFilePage, LocalLibraryOverview};
pub(crate) use matching::MatchTrackDescriptor;
pub use matching::{
    MatchCandidateEvidence, MatchOutcome, MatchRelationship, MatchResult, MatchScoreBreakdown,
};
pub use settings::AppSettings;
pub use source::{
    CollectionEntry, SourceAccount, SourceAccountOverview, SourceCollection,
    SourceCollectionEntryView, SourceCollectionItem, SourceCollectionListPage,
    SourceCollectionPage, SourceCollectionSummary, SourceTrack, SourceTrackView,
    SpotifySourceOverview,
};
