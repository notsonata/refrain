use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use crate::{
    app::AppState,
    db::{Database, DatabaseError},
    domain::{MatchOutcome, MatchTrackDescriptor, ReconciliationCounts, SyncRun, SyncRunPage},
    local_library::scan_library,
    matching::MatcherIndex,
    saved_albums::{
        cancel_spotify_saved_album_refresh, prepare_spotify_saved_album_refresh,
        refresh_spotify_saved_albums,
    },
    source_sync::{SourceRefreshControl, refresh_spotify_source as refresh_source},
    spotify::SpotifyClient,
};

#[derive(Debug, Default)]
pub struct SyncCoordinator {
    active_run_id: Mutex<Option<i64>>,
    cancelled: AtomicBool,
}

#[derive(Debug)]
enum SyncBegin {
    Existing(i64),
    Started(SyncLease),
}

#[derive(Debug)]
struct SyncLease {
    coordinator: Arc<SyncCoordinator>,
    run_id: i64,
}

impl SyncLease {
    fn run_id(&self) -> i64 {
        self.run_id
    }
}

impl Drop for SyncLease {
    fn drop(&mut self) {
        if let Ok(mut active) = self.coordinator.active_run_id.lock()
            && active.as_ref() == Some(&self.run_id)
        {
            *active = None;
            self.coordinator.cancelled.store(false, Ordering::Release);
        }
    }
}

impl SyncCoordinator {
    fn begin(
        self: &Arc<Self>,
        database: &Database,
        trigger: &str,
    ) -> Result<SyncBegin, DatabaseError> {
        let mut active = self.active_run_id.lock().map_err(|_| {
            DatabaseError::InvalidState("sync coordinator lock is poisoned".into())
        })?;
        if let Some(run_id) = *active {
            return Ok(SyncBegin::Existing(run_id));
        }

        let run = database.create_sync_run(trigger)?;
        *active = Some(run.id);
        self.cancelled.store(false, Ordering::Release);
        Ok(SyncBegin::Started(SyncLease {
            coordinator: Arc::clone(self),
            run_id: run.id,
        }))
    }

    pub fn cancel(&self) -> bool {
        let running = self
            .active_run_id
            .lock()
            .map(|active| active.is_some())
            .unwrap_or(false);
        if running {
            self.cancelled.store(true, Ordering::Release);
        }
        running
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

#[derive(Debug)]
enum SyncExecutionFailure {
    Cancelled(ReconciliationCounts),
    Failed {
        counts: ReconciliationCounts,
        message: String,
    },
}

#[derive(Debug)]
enum ReconciliationFailure {
    Cancelled(ReconciliationCounts),
    Database(DatabaseError),
    InvalidState(String),
}

impl From<DatabaseError> for ReconciliationFailure {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}

#[tauri::command]
pub async fn start_sync(
    trigger: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<SyncRun, String> {
    let trigger = trigger.unwrap_or_else(|| "manual".into());
    if !matches!(trigger.as_str(), "manual" | "startup" | "scheduled") {
        return Err("Sync trigger must be manual, startup, or scheduled.".into());
    }

    let begin = state
        .sync
        .begin(&state.database, &trigger)
        .map_err(|error| sync_database_error("start sync", error))?;
    let lease = match begin {
        SyncBegin::Existing(run_id) => {
            return state
                .database
                .sync_run(run_id)
                .map_err(|error| sync_database_error("load active sync", error))?
                .ok_or_else(|| "The active sync run could not be loaded.".to_owned());
        }
        SyncBegin::Started(lease) => lease,
    };

    let run_id = lease.run_id();
    let database = Arc::clone(&state.database);
    let failure_database = Arc::clone(&state.database);
    let spotify = Arc::clone(&state.spotify);
    let source_refresh = Arc::clone(&state.source_refresh);
    let sync = Arc::clone(&state.sync);

    match tauri::async_runtime::spawn_blocking(move || {
        let _lease = lease;
        execute_sync(
            &database,
            spotify,
            &source_refresh,
            &sync,
            run_id,
        )
    })
    .await
    {
        Ok(result) => result,
        Err(error) => {
            tracing::error!(%error, run_id, "sync worker failed");
            let message = "Synchronization stopped unexpectedly.";
            let _ = failure_database.finish_sync_run(
                run_id,
                "failed",
                ReconciliationCounts::default(),
                Some(message),
            );
            failure_database
                .sync_run(run_id)
                .map_err(|database_error| sync_database_error("load failed sync", database_error))?
                .ok_or_else(|| message.to_owned())
        }
    }
}

#[tauri::command]
pub fn cancel_sync(state: tauri::State<'_, AppState>) -> bool {
    let cancelled = state.sync.cancel();
    if cancelled {
        state.source_refresh.cancel();
        cancel_spotify_saved_album_refresh();
    }
    cancelled
}

#[tauri::command]
pub fn get_sync_run(
    run_id: i64,
    state: tauri::State<'_, AppState>,
) -> Result<Option<SyncRun>, String> {
    state
        .database
        .sync_run(run_id)
        .map_err(|error| sync_database_error("load sync run", error))
}

#[tauri::command]
pub fn list_sync_runs(
    offset: u32,
    limit: u32,
    state: tauri::State<'_, AppState>,
) -> Result<SyncRunPage, String> {
    state
        .database
        .sync_runs_page(offset, limit)
        .map_err(|error| sync_database_error("list sync runs", error))
}

fn execute_sync(
    database: &Database,
    spotify: Arc<SpotifyClient>,
    source_refresh: &Arc<SourceRefreshControl>,
    sync: &SyncCoordinator,
    run_id: i64,
) -> Result<SyncRun, String> {
    let outcome = run_initial_sync(database, spotify, source_refresh, sync, run_id);
    let finish_result = match outcome {
        Ok(counts) => database.finish_sync_run(run_id, "succeeded", counts, None),
        Err(SyncExecutionFailure::Cancelled(counts)) => {
            database.finish_sync_run(run_id, "cancelled", counts, None)
        }
        Err(SyncExecutionFailure::Failed { counts, message }) => {
            database.finish_sync_run(run_id, "failed", counts, Some(&message))
        }
    };
    finish_result.map_err(|error| sync_database_error("finish sync run", error))?;
    database
        .sync_run(run_id)
        .map_err(|error| sync_database_error("load completed sync", error))?
        .ok_or_else(|| "Completed sync run could not be loaded.".to_owned())
}

fn run_initial_sync(
    database: &Database,
    spotify: Arc<SpotifyClient>,
    source_refresh: &Arc<SourceRefreshControl>,
    sync: &SyncCoordinator,
    run_id: i64,
) -> Result<ReconciliationCounts, SyncExecutionFailure> {
    let empty_counts = ReconciliationCounts::default();
    if sync.is_cancelled() {
        return Err(SyncExecutionFailure::Cancelled(empty_counts));
    }

    database
        .update_sync_run_phase(run_id, "refreshSource")
        .map_err(|error| failed(empty_counts, "update sync phase", error))?;
    let settings = database
        .get_settings()
        .map_err(|error| failed(empty_counts, "load sync settings", error))?;
    let client_id = settings.spotify_client_id.ok_or_else(|| SyncExecutionFailure::Failed {
        counts: empty_counts,
        message: "Connect Spotify before starting synchronization.".into(),
    })?;
    let source_guard = source_refresh.begin().map_err(|error| {
        if sync.is_cancelled() {
            SyncExecutionFailure::Cancelled(empty_counts)
        } else {
            SyncExecutionFailure::Failed {
                counts: empty_counts,
                message: error.message,
            }
        }
    })?;
    prepare_spotify_saved_album_refresh();
    let source_result = refresh_source(
        database,
        Arc::clone(&spotify),
        &client_id,
        source_refresh,
        |_| {},
    );
    if let Err(error) = source_result {
        return if sync.is_cancelled() || error.code == "refreshCancelled" {
            Err(SyncExecutionFailure::Cancelled(empty_counts))
        } else {
            Err(SyncExecutionFailure::Failed {
                counts: empty_counts,
                message: error.message,
            })
        };
    }
    if let Err(error) = refresh_spotify_saved_albums(database, spotify, &client_id) {
        return if sync.is_cancelled() || error.code == "refreshCancelled" {
            Err(SyncExecutionFailure::Cancelled(empty_counts))
        } else {
            Err(SyncExecutionFailure::Failed {
                counts: empty_counts,
                message: error.message,
            })
        };
    }

    if sync.is_cancelled() {
        return Err(SyncExecutionFailure::Cancelled(empty_counts));
    }
    database
        .update_sync_run_phase(run_id, "scanLocalLibrary")
        .map_err(|error| failed(empty_counts, "update sync phase", error))?;
    let library_root = settings
        .library_root
        .filter(|root| !root.trim().is_empty())
        .ok_or_else(|| SyncExecutionFailure::Failed {
            counts: empty_counts,
            message: "Choose a local library folder before starting synchronization.".into(),
        })?;
    scan_library(database, Path::new(&library_root), |_| {}).map_err(|error| {
        SyncExecutionFailure::Failed {
            counts: empty_counts,
            message: error.to_string(),
        }
    })?;

    if sync.is_cancelled() {
        return Err(SyncExecutionFailure::Cancelled(empty_counts));
    }
    let reconciliation = run_reconciliation_core(database, run_id, || sync.is_cancelled());
    drop(source_guard);
    match reconciliation {
        Ok(counts) => Ok(counts),
        Err(ReconciliationFailure::Cancelled(counts)) => {
            Err(SyncExecutionFailure::Cancelled(counts))
        }
        Err(ReconciliationFailure::Database(error)) => Err(SyncExecutionFailure::Failed {
            counts: empty_counts,
            message: error.to_string(),
        }),
        Err(ReconciliationFailure::InvalidState(message)) => {
            Err(SyncExecutionFailure::Failed {
                counts: empty_counts,
                message,
            })
        }
    }
}

fn run_reconciliation_core(
    database: &Database,
    run_id: i64,
    mut is_cancelled: impl FnMut() -> bool,
) -> Result<ReconciliationCounts, ReconciliationFailure> {
    let mut counts = ReconciliationCounts::default();
    database.update_sync_run_phase(run_id, "resolveExistingLinks")?;
    if is_cancelled() {
        return Err(ReconciliationFailure::Cancelled(counts));
    }
    database.ensure_library_tracks_for_present_local_files()?;

    let source_track_ids = database.accessible_source_track_ids()?;
    let library_tracks = database.library_match_tracks()?;
    let matcher = MatcherIndex::new(library_tracks);
    let mut unresolved = Vec::new();

    database.update_sync_run_phase(run_id, "matchUnresolved")?;
    for source_track_id in source_track_ids {
        if is_cancelled() {
            return Err(ReconciliationFailure::Cancelled(counts));
        }
        let source = database
            .source_match_track(source_track_id)?
            .ok_or_else(|| {
                ReconciliationFailure::InvalidState(format!(
                    "source track {source_track_id} disappeared during reconciliation"
                ))
            })?;
        let existing_link = database.persisted_track_link(source_track_id)?;
        let rejected = database.rejected_library_track_ids(source_track_id)?;
        let result = matcher.match_track(&source, existing_link.as_ref(), &rejected);

        match result.outcome {
            MatchOutcome::Automatic => {
                let library_track_id = result.selected_library_track_id.ok_or_else(|| {
                    ReconciliationFailure::InvalidState(
                        "automatic match did not select a library track".into(),
                    )
                })?;
                if existing_link.is_none() {
                    database.persist_track_link(
                        source_track_id,
                        library_track_id,
                        result.method.as_deref().unwrap_or("metadata"),
                        result.confidence.unwrap_or_default(),
                    )?;
                }
                classify_linked_track(database, library_track_id, &mut counts)?;
            }
            MatchOutcome::Review => counts.needs_review += 1,
            MatchOutcome::Unresolved => unresolved.push(source),
        }
    }

    database.update_sync_run_phase(run_id, "createMissing")?;
    let mut created_by_isrc: HashMap<String, Vec<MatchTrackDescriptor>> = HashMap::new();
    for source in unresolved {
        if is_cancelled() {
            return Err(ReconciliationFailure::Cancelled(counts));
        }

        let isrc_key = normalized_isrc(source.isrc.as_deref());
        if let Some(candidates) = isrc_key
            .as_ref()
            .and_then(|key| created_by_isrc.get(key))
            .filter(|candidates| !candidates.is_empty())
        {
            let provisional = MatcherIndex::new(candidates.clone()).match_track(
                &source,
                None,
                &HashSet::new(),
            );
            if provisional.outcome == MatchOutcome::Automatic
                && let Some(library_track_id) = provisional.selected_library_track_id
            {
                database.persist_track_link(
                    source.id,
                    library_track_id,
                    provisional.method.as_deref().unwrap_or("isrc"),
                    provisional.confidence.unwrap_or_default(),
                )?;
                counts.missing += 1;
                continue;
            }
        }

        let library_track_id = database.create_library_track_from_source(source.id)?;
        database.persist_track_link(source.id, library_track_id, "existing", 10_000)?;
        if let Some(key) = isrc_key {
            let mut descriptor = source.clone();
            descriptor.id = library_track_id;
            created_by_isrc.entry(key).or_default().push(descriptor);
        }
        counts.missing += 1;
    }

    Ok(counts)
}

fn classify_linked_track(
    database: &Database,
    library_track_id: i64,
    counts: &mut ReconciliationCounts,
) -> Result<(), DatabaseError> {
    if database.library_track_has_preferred_present_file(library_track_id)? {
        counts.matched += 1;
    } else {
        counts.missing += 1;
    }
    Ok(())
}

fn normalized_isrc(value: Option<&str>) -> Option<String> {
    value
        .map(|value| {
            value
                .chars()
                .filter(|character| character.is_ascii_alphanumeric())
                .flat_map(char::to_uppercase)
                .collect::<String>()
        })
        .filter(|value| !value.is_empty())
}

fn failed(
    counts: ReconciliationCounts,
    operation: &str,
    error: DatabaseError,
) -> SyncExecutionFailure {
    SyncExecutionFailure::Failed {
        counts,
        message: format!("Failed to {operation}: {error}"),
    }
}

fn sync_database_error(operation: &str, error: DatabaseError) -> String {
    tracing::error!(%error, operation, "sync persistence operation failed");
    format!("Failed to {operation}.")
}

#[cfg(test)]
mod tests {
    use std::{
        cell::Cell,
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering as AtomicOrdering},
    };

    use crate::{
        db::LocalFileWrite,
        domain::{CollectionEntry, SourceAccount, SourceCollection, SourceTrack},
    };

    use super::*;

    static NEXT_DATABASE_ID: AtomicU64 = AtomicU64::new(0);

    struct TestDatabasePath(PathBuf);

    impl TestDatabasePath {
        fn new(name: &str) -> Self {
            let id = NEXT_DATABASE_ID.fetch_add(1, AtomicOrdering::Relaxed);
            Self(std::env::temp_dir().join(format!(
                "refrain-reconciliation-{name}-{}-{id}.sqlite3",
                std::process::id()
            )))
        }
    }

    impl Drop for TestDatabasePath {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
            let _ = fs::remove_file(self.0.with_extension("sqlite3-wal"));
            let _ = fs::remove_file(self.0.with_extension("sqlite3-shm"));
        }
    }

    fn source_account() -> SourceAccount {
        SourceAccount {
            provider: "spotify".into(),
            provider_account_id: "account".into(),
            display_name: Some("Listener".into()),
            client_id: "client".into(),
        }
    }

    fn collection(account_id: i64, provider_id: &str) -> SourceCollection {
        SourceCollection {
            source_account_id: account_id,
            provider_collection_id: provider_id.into(),
            kind: "playlist".into(),
            name: provider_id.into(),
            snapshot_id: None,
            owner_provider_id: Some("account".into()),
            is_accessible: true,
            access_issue: None,
        }
    }

    fn source_track(provider_id: &str, isrc: &str) -> SourceTrack {
        SourceTrack {
            provider: "spotify".into(),
            provider_track_id: provider_id.into(),
            uri: Some(format!("spotify:track:{provider_id}")),
            isrc: Some(isrc.into()),
            title: "Song".into(),
            normalized_title: "song".into(),
            artists_json: "[\"Artist\"]".into(),
            normalized_artists: "artist".into(),
            album: Some("Album".into()),
            normalized_album: Some("album".into()),
            duration_ms: Some(180_000),
            disc_number: Some(1),
            track_number: Some(1),
            release_year: Some(2026),
            explicit: Some(false),
            version_kind: None,
            version_detail: None,
            image_url: None,
            external_url: None,
        }
    }

    fn entry(position: i64, source_track_id: i64) -> CollectionEntry {
        CollectionEntry {
            position,
            source_track_id: Some(source_track_id),
            provider_item_uri: None,
            item_type: "track".into(),
            added_at: None,
            unavailable_reason: None,
        }
    }

    fn run_core(database: &Database) -> ReconciliationCounts {
        let run = database.create_sync_run("manual").unwrap();
        let counts = run_reconciliation_core(database, run.id, || false).unwrap();
        database
            .finish_sync_run(run.id, "succeeded", counts, None)
            .unwrap();
        counts
    }

    #[test]
    fn repeated_reconciliation_is_idempotent_and_preserves_duplicate_entries() {
        let path = TestDatabasePath::new("idempotent");
        let database = Database::open(path.0.clone()).unwrap();
        let account_id = database.upsert_source_account(&source_account()).unwrap();
        let collection_id = database
            .upsert_source_collection(&collection(account_id, "playlist"))
            .unwrap();
        let first = database
            .upsert_source_track(&source_track("track-a", "USAAA0000001"))
            .unwrap();
        let second = database
            .upsert_source_track(&source_track("track-b", "USAAA0000001"))
            .unwrap();
        database
            .replace_collection_entries(
                collection_id,
                &[entry(0, first), entry(1, first), entry(2, second)],
            )
            .unwrap();

        let first_counts = run_core(&database);
        assert_eq!(first_counts.missing, 2);
        assert_eq!(database.library_track_count().unwrap(), 1);
        let first_link = database.persisted_track_link(first).unwrap().unwrap();
        let second_link = database.persisted_track_link(second).unwrap().unwrap();
        assert_eq!(first_link.library_track_id, second_link.library_track_id);

        let second_counts = run_core(&database);
        assert_eq!(second_counts.missing, 2);
        assert_eq!(database.library_track_count().unwrap(), 1);
        let page = database.source_collection_page(collection_id, 0, 10).unwrap().unwrap();
        assert_eq!(page.entries.len(), 3);
        assert_eq!(page.entries[0].position, 0);
        assert_eq!(page.entries[1].position, 1);
        assert_eq!(page.entries[2].position, 2);
    }

    #[test]
    fn manual_decisions_and_other_collection_references_survive_reconciliation() {
        let path = TestDatabasePath::new("manual");
        let database = Database::open(path.0.clone()).unwrap();
        let account_id = database.upsert_source_account(&source_account()).unwrap();
        let first_collection = database
            .upsert_source_collection(&collection(account_id, "playlist-a"))
            .unwrap();
        let second_collection = database
            .upsert_source_collection(&collection(account_id, "playlist-b"))
            .unwrap();
        let source_track_id = database
            .upsert_source_track(&source_track("track", "USAAA0000002"))
            .unwrap();
        database
            .replace_collection_entries(first_collection, &[entry(0, source_track_id)])
            .unwrap();
        database
            .replace_collection_entries(second_collection, &[entry(0, source_track_id)])
            .unwrap();
        database
            .insert_local_file(&LocalFileWrite {
                path: "/music/song.flac".into(),
                state: "present".into(),
                format: Some("flac".into()),
                file_size: 100,
                modified_at: 1,
                duration_ms: Some(180_000),
                bitrate: None,
                sample_rate: None,
                channels: None,
                content_hash: None,
                tag_title: Some("Song".into()),
                tag_artists: vec!["Artist".into()],
                tag_album: Some("Album".into()),
                tag_isrc: Some("USAAA0000002".into()),
                scan_error: None,
            })
            .unwrap();

        let counts = run_core(&database);
        assert_eq!(counts.matched, 1);
        let mut files = database.local_files_page(0, 10).unwrap();
        let file = files.items.remove(0);
        assert!(file.is_preferred);
        let library_track_id = file.library_track_id.unwrap();
        database
            .confirm_match(source_track_id, library_track_id)
            .unwrap();

        database
            .replace_collection_entries(first_collection, &[])
            .unwrap();
        let counts = run_core(&database);
        assert_eq!(counts.matched, 1);
        let link = database
            .persisted_track_link(source_track_id)
            .unwrap()
            .unwrap();
        assert!(link.confirmed_by_user);
        assert_eq!(link.method, "user");
        assert_eq!(link.library_track_id, library_track_id);
        assert_eq!(
            database
                .source_collection_page(second_collection, 0, 10)
                .unwrap()
                .unwrap()
                .entries
                .len(),
            1
        );
    }

    #[test]
    fn cancellation_preserves_already_committed_missing_identity() {
        let path = TestDatabasePath::new("cancel");
        let database = Database::open(path.0.clone()).unwrap();
        let account_id = database.upsert_source_account(&source_account()).unwrap();
        let collection_id = database
            .upsert_source_collection(&collection(account_id, "playlist"))
            .unwrap();
        let first = database
            .upsert_source_track(&source_track("track-a", "USAAA0000003"))
            .unwrap();
        let second = database
            .upsert_source_track(&source_track("track-b", "USAAA0000004"))
            .unwrap();
        database
            .replace_collection_entries(collection_id, &[entry(0, first), entry(1, second)])
            .unwrap();
        let run = database.create_sync_run("manual").unwrap();
        let checks = Cell::new(0usize);

        let result = run_reconciliation_core(&database, run.id, || {
            let next = checks.get() + 1;
            checks.set(next);
            next >= 5
        });
        let ReconciliationFailure::Cancelled(counts) = result.unwrap_err() else {
            panic!("reconciliation should cancel");
        };
        assert_eq!(counts.missing, 1);
        assert!(database.persisted_track_link(first).unwrap().is_some());
        assert!(database.persisted_track_link(second).unwrap().is_none());
        assert_eq!(database.library_track_count().unwrap(), 1);
    }

    #[test]
    fn coordinator_returns_the_active_run_instead_of_starting_another() {
        let path = TestDatabasePath::new("coordinator");
        let database = Database::open(path.0.clone()).unwrap();
        let coordinator = Arc::new(SyncCoordinator::default());
        let first = coordinator.begin(&database, "manual").unwrap();
        let SyncBegin::Started(lease) = first else {
            panic!("first sync should start");
        };
        let first_id = lease.run_id();
        let second = coordinator.begin(&database, "manual").unwrap();
        assert!(matches!(second, SyncBegin::Existing(id) if id == first_id));
        drop(lease);
        assert!(matches!(
            coordinator.begin(&database, "manual").unwrap(),
            SyncBegin::Started(_)
        ));
    }
}
