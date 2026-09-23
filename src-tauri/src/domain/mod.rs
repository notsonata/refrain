mod settings;
// Milestone 2 establishes these source models ahead of the Spotify sync that
// consumes them in Milestone 4.
#[allow(dead_code)]
mod source;

pub use settings::AppSettings;
pub use source::{CollectionEntry, SourceAccount, SourceCollection, SourceTrack};
