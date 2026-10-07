use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use serde::Serialize;
use tauri::Emitter;

use crate::{
    acquisition::{AcquisitionCoordinator, AcquisitionProvider},
    antra::AntraProvider,
    app::AppState,
    db::{Database, DatabaseError},
    domain::{
        AcquisitionJob, MatchOutcome, MatchTrackDescriptor, ReconciliationCounts, SyncRun,
        SyncRunPage,
    },
    local_library::scan_library,
    matching::MatcherIndex,
    monochrome::MonochromeProvider,
    normalization::{NormalizationError, normalize_resolved_files},
    playlist_sync::sync_managed_local_playlists,
    saved_albums::{
        cancel_spotify_saved_album_refresh, prepare_spotify_saved_album_refresh,
        refresh_spotify_saved_albums,
    },
    sockseek::SockseekManager,
    source_sync::{SourceRefreshControl, refresh_spotify_source as refresh_source},
    spotify::SpotifyClient,
    verification::{continue_downloaded_jobs, downloaded_staging_artifact_exists},
};

pub const SYNC_PROGRESS_EVENT: &str = "library-sync-progress";

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SyncProgress {
    pub run_id: i64,
    pub scope: String,
    pub phase: String,
    pub completed: usize,
    pub total: Option<usize>,
    pub message: String,
}

#[derive(Debug, Default)]
struct SyncExecutionSuccess {
    counts: ReconciliationCounts,
    warnings: Vec<String>,
}

struct AvailableAcquisitionProviders {
    providers: Vec<Arc<dyn AcquisitionProvider>>,
    warnings: Vec<String>,
}

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
        scope: &str,
        trigger: &str,
    ) -> Result<SyncBegin, DatabaseError> {
        let mut active = self
            .active_run_id
            .lock()
            .map_err(|_| DatabaseError::InvalidState("sync coordinator lock is poisoned".into()))?;
        if let Some(run_id) = *active {
            return Ok(SyncBegin::Existing(run_id));
        }

        let run = database.create_sync_run(scope, trigger)?;
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

struct SyncExecutionContext<'a> {
    app: &'a tauri::AppHandle,
    database: &'a Database,
    source_refresh: &'a Arc<SourceRefreshControl>,
    sync: &'a SyncCoordinator,
    monochrome: &'a Arc<MonochromeProvider>,
    antra: &'a Arc<AntraProvider>,
    sockseek: &'a Arc<SockseekManager>,
    app_data_dir: &'a Path,
}

impl From<DatabaseError> for ReconciliationFailure {
    fn from(error: DatabaseError) -> Self {
        Self::Database(error)
    }
}

#[tauri::command]
pub async fn start_sync(
    trigger: Option<String>,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<SyncRun, String> {
    start_scoped_sync("spotify", trigger, app, state).await
}

#[tauri::command]
pub async fn start_local_sync(
    trigger: Option<String>,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<SyncRun, String> {
    start_scoped_sync("local", trigger, app, state).await
}

#[tauri::command]
pub async fn start_spotify_sync(
    trigger: Option<String>,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<SyncRun, String> {
    start_scoped_sync("spotify", trigger, app, state).await
}

async fn start_scoped_sync(
    scope: &'static str,
    trigger: Option<String>,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<SyncRun, String> {
    let trigger = trigger.unwrap_or_else(|| "manual".into());
    if !matches!(trigger.as_str(), "manual" | "startup" | "scheduled") {
        return Err("Sync trigger must be manual, startup, or scheduled.".into());
    }

    let begin = state
        .sync
        .begin(&state.database, scope, &trigger)
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
    let library_lock = Arc::clone(&state.library_lock);
    let monochrome = Arc::clone(&state.monochrome);
    let antra = Arc::clone(&state.antra);
    let sockseek = Arc::clone(&state.sockseek);
    let app_data_dir = state.app_data_dir.clone();

    match tauri::async_runtime::spawn_blocking(move || {
        let _lease = lease;
        let _library_guard = match library_lock.write() {
            Ok(guard) => guard,
            Err(_) => {
                let message = "Library mutation lock is poisoned.";
                let _ = database.finish_sync_run(
                    run_id,
                    "failed",
                    ReconciliationCounts::default(),
                    Some(message),
                );
                return database
                    .sync_run(run_id)
                    .map_err(|error| sync_database_error("load failed sync", error))?
                    .ok_or_else(|| message.to_owned());
            }
        };
        let context = SyncExecutionContext {
            app: &app,
            database: &database,
            source_refresh: &source_refresh,
            sync: &sync,
            monochrome: &monochrome,
            antra: &antra,
            sockseek: &sockseek,
            app_data_dir: &app_data_dir,
        };
        execute_sync(&context, spotify, run_id, scope)
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
    scope: Option<String>,
    offset: u32,
    limit: u32,
    state: tauri::State<'_, AppState>,
) -> Result<SyncRunPage, String> {
    state
        .database
        .sync_runs_page(scope.as_deref(), offset, limit)
        .map_err(|error| sync_database_error("list sync runs", error))
}

fn execute_sync(
    context: &SyncExecutionContext<'_>,
    spotify: Arc<SpotifyClient>,
    run_id: i64,
    scope: &str,
) -> Result<SyncRun, String> {
    let outcome = if scope == "local" {
        run_local_sync(context, run_id)
    } else {
        run_spotify_sync(context, spotify, run_id)
    };
    let finish_result =
        match outcome {
            Ok(success) => {
                let status = completion_status(success.counts);
                let warning_message = aggregate_warning_messages(&success.warnings);
                context.database.finish_sync_run(
                    run_id,
                    status,
                    success.counts,
                    warning_message.as_deref(),
                )
            }
            Err(SyncExecutionFailure::Cancelled(counts)) => {
                context
                    .database
                    .finish_sync_run(run_id, "cancelled", counts, None)
            }
            Err(SyncExecutionFailure::Failed { counts, message }) => context
                .database
                .finish_sync_run(run_id, "failed", counts, Some(&message)),
        };
    finish_result.map_err(|error| sync_database_error("finish sync run", error))?;
    emit_sync_progress(
        context,
        run_id,
        scope,
        "complete",
        1,
        Some(1),
        "Synchronization finished",
    );
    context
        .database
        .sync_run(run_id)
        .map_err(|error| sync_database_error("load completed sync", error))?
        .ok_or_else(|| "Completed sync run could not be loaded.".to_owned())
}

fn source_membership_delta(before: &[i64], after: &[i64]) -> (i64, i64) {
    let before = before.iter().copied().collect::<HashSet<_>>();
    let after = after.iter().copied().collect::<HashSet<_>>();
    let added = after.difference(&before).count() as i64;
    let removed = before.difference(&after).count() as i64;
    (added, removed)
}

fn emit_sync_progress(
    context: &SyncExecutionContext<'_>,
    run_id: i64,
    scope: &str,
    phase: &str,
    completed: usize,
    total: Option<usize>,
    message: impl Into<String>,
) {
    let progress = SyncProgress {
        run_id,
        scope: scope.into(),
        phase: phase.into(),
        completed,
        total,
        message: message.into(),
    };
    if let Err(error) = context.app.emit(SYNC_PROGRESS_EVENT, progress) {
        tracing::warn!(%error, run_id, phase, "sync progress event failed");
    }
}

fn update_sync_phase(
    context: &SyncExecutionContext<'_>,
    run_id: i64,
    scope: &str,
    phase: &str,
    message: impl Into<String>,
) -> Result<(), DatabaseError> {
    context.database.update_sync_run_phase(run_id, phase)?;
    emit_sync_progress(context, run_id, scope, phase, 0, None, message);
    Ok(())
}

fn sync_phase_message(scope: &str, phase: &str) -> &'static str {
    match (scope, phase) {
        ("local", "scanLocalLibrary") | ("spotify", "scanLocalLibrary") => "Scanning local library",
        ("local", "compareWithSpotify") => "Comparing local tracks with Spotify",
        ("spotify", "refreshSource") => "Refreshing Spotify source state",
        ("spotify", "resolveExistingLinks") => "Resolving existing track links",
        ("spotify", "matchUnresolved") => "Matching unresolved tracked music",
        ("spotify", "createMissing") => "Identifying missing tracked music",
        ("spotify", "resolvePlaylists") => "Updating tracked playlist mirrors",
        ("spotify", "acquireMissing") => "Acquiring missing tracked audio",
        ("spotify", "verifyImports") => "Verifying and importing downloaded audio",
        ("spotify", "postAcquisitionReconcile") => "Reconciling imported audio",
        (_, "normalizeFiles") => "Normalizing library files",
        _ => "Synchronizing library",
    }
}

fn completion_status(counts: ReconciliationCounts) -> &'static str {
    if counts.acquisition_failed > 0 {
        "partial"
    } else {
        "succeeded"
    }
}

fn acquisition_outcome_counts(jobs: &[AcquisitionJob]) -> (i64, i64) {
    let needs_resolution = jobs
        .iter()
        .filter(|job| job.stage.as_deref() == Some("needsResolution"))
        .count() as i64;
    let failed = jobs
        .iter()
        .filter(|job| job.status == "failed" && job.stage.as_deref() != Some("needsResolution"))
        .count() as i64;
    (needs_resolution, failed)
}

fn aggregate_warning_messages(messages: &[String]) -> Option<String> {
    let mut seen = HashSet::new();
    let mut unique = Vec::new();
    for message in messages {
        let message = message.trim();
        if message.is_empty() || !seen.insert(message.to_owned()) {
            continue;
        }
        unique.push(message.to_owned());
    }
    (!unique.is_empty()).then(|| unique.join("; "))
}

fn run_spotify_sync(
    context: &SyncExecutionContext<'_>,
    spotify: Arc<SpotifyClient>,
    run_id: i64,
) -> Result<SyncExecutionSuccess, SyncExecutionFailure> {
    let database = context.database;
    let source_refresh = context.source_refresh;
    let sync = context.sync;
    let empty_counts = ReconciliationCounts::default();
    if sync.is_cancelled() {
        return Err(SyncExecutionFailure::Cancelled(empty_counts));
    }

    update_sync_phase(
        context,
        run_id,
        "spotify",
        "refreshSource",
        "Refreshing Spotify source state",
    )
    .map_err(|error| failed(empty_counts, "update sync phase", error))?;
    let settings = database
        .get_settings()
        .map_err(|error| failed(empty_counts, "load sync settings", error))?;
    let client_id = settings
        .spotify_client_id
        .ok_or_else(|| SyncExecutionFailure::Failed {
            counts: empty_counts,
            message: "Connect Spotify before starting synchronization.".into(),
        })?;
    let source_track_ids_before = database
        .accessible_source_track_ids()
        .map_err(|error| failed(empty_counts, "load source state", error))?;
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
        |progress| {
            emit_sync_progress(
                context,
                run_id,
                "spotify",
                "refreshSource",
                progress.completed,
                progress.total,
                progress.message,
            );
        },
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
    let source_track_ids_after = database
        .accessible_source_track_ids()
        .map_err(|error| failed(empty_counts, "load refreshed source state", error))?;
    let (source_added, source_removed) =
        source_membership_delta(&source_track_ids_before, &source_track_ids_after);

    if sync.is_cancelled() {
        return Err(SyncExecutionFailure::Cancelled(empty_counts));
    }
    update_sync_phase(
        context,
        run_id,
        "spotify",
        "scanLocalLibrary",
        "Scanning local library",
    )
    .map_err(|error| failed(empty_counts, "update sync phase", error))?;
    let library_root = settings
        .library_root
        .filter(|root| !root.trim().is_empty())
        .ok_or_else(|| SyncExecutionFailure::Failed {
            counts: empty_counts,
            message: "Choose a local library folder before starting synchronization.".into(),
        })?;
    scan_library(database, Path::new(&library_root), |progress| {
        emit_sync_progress(
            context,
            run_id,
            "spotify",
            "scanLocalLibrary",
            progress.completed,
            progress.total,
            progress.message,
        );
    })
    .map_err(|error| SyncExecutionFailure::Failed {
        counts: empty_counts,
        message: error.to_string(),
    })?;

    if sync.is_cancelled() {
        return Err(SyncExecutionFailure::Cancelled(empty_counts));
    }
    let reconciliation = reconcile_spotify_source_state(
        database,
        || sync.is_cancelled(),
        |phase| {
            update_sync_phase(
                context,
                run_id,
                "spotify",
                phase,
                sync_phase_message("spotify", phase),
            )
        },
    );
    drop(source_guard);
    let mut counts = match reconciliation {
        Ok(counts) => counts,
        Err(ReconciliationFailure::Cancelled(counts)) => {
            return Err(SyncExecutionFailure::Cancelled(counts));
        }
        Err(ReconciliationFailure::Database(error)) => {
            return Err(SyncExecutionFailure::Failed {
                counts: empty_counts,
                message: error.to_string(),
            });
        }
        Err(ReconciliationFailure::InvalidState(message)) => {
            return Err(SyncExecutionFailure::Failed {
                counts: empty_counts,
                message,
            });
        }
    };
    counts.source_added = source_added;
    counts.source_removed = source_removed;

    if sync.is_cancelled() {
        return Err(SyncExecutionFailure::Cancelled(counts));
    }

    let mut warnings = Vec::new();
    if settings.acquisition_enabled {
        update_sync_phase(
            context,
            run_id,
            "spotify",
            "acquireMissing",
            "Acquiring missing tracked audio",
        )
        .map_err(|error| failed(counts, "update sync phase", error))?;

        for job in database
            .downloaded_acquisition_jobs_missing_local()
            .map_err(|error| failed(counts, "load downloaded acquisitions", error))?
        {
            if downloaded_staging_artifact_exists(&job) {
                continue;
            }
            database
                .finish_acquisition_job(
                    job.id,
                    "failed",
                    Some("stagingMissing"),
                    Some(
                        "The downloaded staging audio is no longer available. Refrain will acquire it again.",
                    ),
                )
                .map_err(|error| failed(counts, "prepare acquisition recovery", error))?;
        }

        let available = acquisition_providers(context, &settings.acquisition_providers)
            .map_err(|message| SyncExecutionFailure::Failed { counts, message })?;
        warnings.extend(available.warnings);
        let acquisition_jobs = AcquisitionCoordinator
            .run_missing_chain(database, context.app_data_dir, &available.providers, || {
                sync.is_cancelled()
            })
            .map_err(|error| SyncExecutionFailure::Failed {
                counts,
                message: error.to_string(),
            })?;
        let (acquisition_reviews, provider_failures) =
            acquisition_outcome_counts(&acquisition_jobs);
        warnings.extend(
            acquisition_jobs
                .iter()
                .filter(|job| job.status == "failed")
                .filter_map(|job| job.error_message.as_deref())
                .map(str::to_owned),
        );
        let staged_job_ids = database
            .downloaded_acquisition_jobs_missing_local()
            .map_err(|error| failed(counts, "load downloaded acquisitions", error))?
            .into_iter()
            .map(|job| job.id)
            .collect::<Vec<_>>();
        let mut imported = 0_usize;
        let mut import_failures = 0_i64;
        if !staged_job_ids.is_empty() {
            update_sync_phase(
                context,
                run_id,
                "spotify",
                "verifyImports",
                "Verifying and importing downloaded audio",
            )
            .map_err(|error| failed(counts, "update sync phase", error))?;
            for job_id in staged_job_ids {
                if sync.is_cancelled() {
                    return Err(SyncExecutionFailure::Cancelled(counts));
                }
                let summary =
                    continue_downloaded_jobs(database, Path::new(&library_root), &[job_id]);
                imported = imported.saturating_add(summary.imported);
                import_failures += i64::try_from(summary.failed).unwrap_or(i64::MAX);
                warnings.extend(
                    summary
                        .results
                        .into_iter()
                        .filter_map(|result| result.error_message),
                );
            }
        }
        update_sync_phase(
            context,
            run_id,
            "spotify",
            "postAcquisitionReconcile",
            "Reconciling imported audio",
        )
        .map_err(|error| failed(counts, "update sync phase", error))?;
        let mut final_counts = reconcile_spotify_source_state(
            database,
            || sync.is_cancelled(),
            |_| Ok(()),
        )
        .map_err(|error| match error {
            ReconciliationFailure::Cancelled(current) => SyncExecutionFailure::Cancelled(current),
            ReconciliationFailure::Database(error) => {
                failed(counts, "reconcile imported audio", error)
            }
            ReconciliationFailure::InvalidState(message) => {
                SyncExecutionFailure::Failed { counts, message }
            }
        })?;
        final_counts.source_added = source_added;
        final_counts.source_removed = source_removed;
        final_counts.needs_review = final_counts
            .needs_review
            .saturating_add(acquisition_reviews);
        final_counts.acquisition_failed = provider_failures.saturating_add(import_failures);
        counts = final_counts;
        tracing::info!(
            run_id,
            imported,
            import_failed = import_failures,
            needs_resolution = acquisition_reviews,
            failed = counts.acquisition_failed,
            cancelled = acquisition_jobs
                .iter()
                .filter(|job| job.status == "cancelled")
                .count(),
            "acquisition and import phases completed"
        );
        if sync.is_cancelled() {
            return Err(SyncExecutionFailure::Cancelled(counts));
        }
    }

    update_sync_phase(
        context,
        run_id,
        "spotify",
        "normalizeFiles",
        "Normalizing library files",
    )
    .map_err(|error| failed(counts, "update sync phase", error))?;
    match normalize_resolved_files(database, Path::new(&library_root), || sync.is_cancelled()) {
        Ok(summary) => {
            tracing::info!(
                run_id,
                moved = summary.moved,
                unchanged = summary.unchanged,
                "filesystem normalization completed"
            );
            Ok(SyncExecutionSuccess { counts, warnings })
        }
        Err(NormalizationError::Cancelled) => Err(SyncExecutionFailure::Cancelled(counts)),
        Err(error) => Err(SyncExecutionFailure::Failed {
            counts,
            message: error.to_string(),
        }),
    }
}

fn acquisition_provider(
    context: &SyncExecutionContext<'_>,
    provider_id: &str,
) -> Result<Arc<dyn AcquisitionProvider>, String> {
    match provider_id {
        "monochrome" => {
            let provider: Arc<dyn AcquisitionProvider> = context.monochrome.clone();
            Ok(provider)
        }
        "antra" => {
            let provider: Arc<dyn AcquisitionProvider> = context.antra.clone();
            Ok(provider)
        }
        "sockseek" => context
            .sockseek
            .provider(context.app, context.app_data_dir)
            .map(|provider| provider as Arc<dyn AcquisitionProvider>)
            .map_err(|error| error.to_string()),
        _ => Err(format!("Unsupported acquisition provider: {provider_id}")),
    }
}

fn acquisition_providers(
    context: &SyncExecutionContext<'_>,
    provider_ids: &[String],
) -> Result<AvailableAcquisitionProviders, String> {
    let mut providers = Vec::new();
    let mut failures = Vec::new();
    for provider_id in provider_ids {
        match acquisition_provider(context, provider_id) {
            Ok(provider) => providers.push(provider),
            Err(error) => failures.push(format!("{provider_id}: {error}")),
        }
    }
    if providers.is_empty() {
        return Err(if failures.is_empty() {
            "No acquisition providers are configured.".into()
        } else {
            format!(
                "No configured acquisition provider could start: {}",
                failures.join("; ")
            )
        });
    }
    if !failures.is_empty() {
        tracing::warn!(failures = %failures.join("; "), "some acquisition providers are unavailable");
    }
    Ok(AvailableAcquisitionProviders {
        providers,
        warnings: failures,
    })
}

fn run_local_sync(
    context: &SyncExecutionContext<'_>,
    run_id: i64,
) -> Result<SyncExecutionSuccess, SyncExecutionFailure> {
    let database = context.database;
    let sync = context.sync;
    let empty_counts = ReconciliationCounts::default();
    let settings = database
        .get_settings()
        .map_err(|error| failed(empty_counts, "load sync settings", error))?;
    let library_root = settings
        .library_root
        .filter(|root| !root.trim().is_empty())
        .ok_or_else(|| SyncExecutionFailure::Failed {
            counts: empty_counts,
            message: "Choose a local library folder before starting synchronization.".into(),
        })?;

    update_sync_phase(
        context,
        run_id,
        "local",
        "scanLocalLibrary",
        "Scanning local library",
    )
    .map_err(|error| failed(empty_counts, "update sync phase", error))?;
    scan_library(database, Path::new(&library_root), |progress| {
        emit_sync_progress(
            context,
            run_id,
            "local",
            "scanLocalLibrary",
            progress.completed,
            progress.total,
            progress.message,
        );
    })
    .map_err(|error| SyncExecutionFailure::Failed {
        counts: empty_counts,
        message: error.to_string(),
    })?;
    if sync.is_cancelled() {
        return Err(SyncExecutionFailure::Cancelled(empty_counts));
    }

    let counts = reconcile_local_source_state(
        database,
        || sync.is_cancelled(),
        |phase| {
            update_sync_phase(
                context,
                run_id,
                "local",
                phase,
                sync_phase_message("local", phase),
            )
        },
    )
    .map_err(|error| match error {
        ReconciliationFailure::Cancelled(counts) => SyncExecutionFailure::Cancelled(counts),
        ReconciliationFailure::Database(error) => SyncExecutionFailure::Failed {
            counts: empty_counts,
            message: error.to_string(),
        },
        ReconciliationFailure::InvalidState(message) => SyncExecutionFailure::Failed {
            counts: empty_counts,
            message,
        },
    })?;

    update_sync_phase(
        context,
        run_id,
        "local",
        "normalizeFiles",
        "Normalizing library files",
    )
    .map_err(|error| failed(counts, "update sync phase", error))?;
    match normalize_resolved_files(database, Path::new(&library_root), || sync.is_cancelled()) {
        Ok(summary) => {
            tracing::info!(
                run_id,
                moved = summary.moved,
                unchanged = summary.unchanged,
                "local filesystem normalization completed"
            );
            Ok(SyncExecutionSuccess {
                counts,
                warnings: Vec::new(),
            })
        }
        Err(NormalizationError::Cancelled) => Err(SyncExecutionFailure::Cancelled(counts)),
        Err(error) => Err(SyncExecutionFailure::Failed {
            counts,
            message: error.to_string(),
        }),
    }
}

fn reconcile_local_source_state(
    database: &Database,
    mut is_cancelled: impl FnMut() -> bool,
    mut update_phase: impl FnMut(&str) -> Result<(), DatabaseError>,
) -> Result<ReconciliationCounts, ReconciliationFailure> {
    let mut counts = ReconciliationCounts::default();
    update_phase("compareWithSpotify")?;
    database.ensure_library_tracks_for_present_local_files()?;
    let source_track_ids = database.accessible_source_track_ids()?;
    let library_tracks = database.present_library_match_tracks()?;
    let matcher = MatcherIndex::new(library_tracks);

    for source_track_id in source_track_ids {
        if is_cancelled() {
            return Err(ReconciliationFailure::Cancelled(counts));
        }
        let source = database
            .source_match_track(source_track_id)?
            .ok_or_else(|| {
                ReconciliationFailure::InvalidState(format!(
                    "source track {source_track_id} disappeared during local comparison"
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
                let link_changed = existing_link
                    .as_ref()
                    .map(|link| link.library_track_id != library_track_id)
                    .unwrap_or(true);
                if link_changed {
                    database.persist_track_link(
                        source_track_id,
                        library_track_id,
                        result.method.as_deref().unwrap_or("metadata"),
                        result.confidence.unwrap_or_default(),
                    )?;
                }
                counts.matched += 1;
            }
            MatchOutcome::Review => counts.needs_review += 1,
            MatchOutcome::Unresolved => {}
        }
    }

    Ok(counts)
}

#[cfg(test)]
fn run_spotify_reconciliation_core(
    database: &Database,
    run_id: i64,
    is_cancelled: impl FnMut() -> bool,
) -> Result<ReconciliationCounts, ReconciliationFailure> {
    reconcile_spotify_source_state(database, is_cancelled, |phase| {
        database.update_sync_run_phase(run_id, phase)
    })
}

pub(crate) fn reconcile_refreshed_spotify_source(
    database: &Database,
) -> Result<ReconciliationCounts, DatabaseError> {
    match reconcile_spotify_source_state(database, || false, |_| Ok(())) {
        Ok(counts) => Ok(counts),
        Err(ReconciliationFailure::Database(error)) => Err(error),
        Err(ReconciliationFailure::InvalidState(message)) => {
            Err(DatabaseError::InvalidState(message))
        }
        Err(ReconciliationFailure::Cancelled(_)) => Err(DatabaseError::InvalidState(
            "Spotify source reconciliation was unexpectedly cancelled.".into(),
        )),
    }
}

fn reconcile_spotify_source_state(
    database: &Database,
    mut is_cancelled: impl FnMut() -> bool,
    mut update_phase: impl FnMut(&str) -> Result<(), DatabaseError>,
) -> Result<ReconciliationCounts, ReconciliationFailure> {
    let mut counts = ReconciliationCounts::default();
    update_phase("resolveExistingLinks")?;
    if is_cancelled() {
        return Err(ReconciliationFailure::Cancelled(counts));
    }
    database.ensure_library_tracks_for_present_local_files()?;

    let source_track_ids = database.tracked_source_track_ids()?;
    let library_tracks = database.library_match_tracks()?;
    let matcher = MatcherIndex::new(library_tracks);
    let mut unresolved = Vec::new();

    update_phase("matchUnresolved")?;
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

    update_phase("createMissing")?;
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
            let provisional =
                MatcherIndex::new(candidates.clone()).match_track(&source, None, &HashSet::new());
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

    update_phase("resolvePlaylists")?;
    if is_cancelled() {
        return Err(ReconciliationFailure::Cancelled(counts));
    }
    database.sync_tracked_spotify_playlist_mirrors()?;
    sync_managed_local_playlists(database)?;

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
            image_url: None,
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
            image_url: None,
            external_url: None,
            album_metadata: None,
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
        let run = database.create_sync_run("spotify", "manual").unwrap();
        let counts = run_spotify_reconciliation_core(database, run.id, || false).unwrap();
        database
            .finish_sync_run(run.id, "succeeded", counts, None)
            .unwrap();
        counts
    }

    fn acquisition_job_with_outcome(status: &str, stage: &str) -> AcquisitionJob {
        AcquisitionJob {
            id: 1,
            library_track_id: 1,
            track_title: "Song".into(),
            track_artists: vec!["Artist".into()],
            provider: "test".into(),
            provider_job_id: None,
            status: status.into(),
            stage: Some(stage.into()),
            attempt: 1,
            candidate: None,
            candidates: Vec::new(),
            staging_path: None,
            error_code: None,
            error_message: None,
            created_at: 0,
            started_at: None,
            finished_at: None,
            updated_at: 0,
        }
    }

    #[test]
    fn acquisition_resolution_rows_are_review_not_provider_failures() {
        let jobs = vec![
            acquisition_job_with_outcome("failed", "needsResolution"),
            acquisition_job_with_outcome("failed", "failed"),
            acquisition_job_with_outcome("staged", "downloaded"),
        ];

        assert_eq!(acquisition_outcome_counts(&jobs), (1, 1));
    }

    #[test]
    fn acquisition_failures_make_sync_partial_and_persist_the_count() {
        let path = TestDatabasePath::new("acquisition-failures");
        let database = Database::open(path.0.clone()).unwrap();
        let run = database.create_sync_run("spotify", "manual").unwrap();
        let counts = ReconciliationCounts {
            acquisition_failed: 5,
            ..ReconciliationCounts::default()
        };

        assert_eq!(completion_status(counts), "partial");
        database
            .finish_sync_run(run.id, completion_status(counts), counts, None)
            .unwrap();

        let completed = database.sync_run(run.id).unwrap().unwrap();
        assert_eq!(completed.status, "partial");
        assert_eq!(completed.acquisition_failed, 5);
    }

    #[test]
    fn sync_run_persists_source_change_counts() {
        let path = TestDatabasePath::new("source-change-counts");
        let database = Database::open(path.0.clone()).unwrap();
        let run = database.create_sync_run("spotify", "manual").unwrap();
        let counts = ReconciliationCounts {
            source_added: 3,
            source_removed: 2,
            ..ReconciliationCounts::default()
        };

        database
            .finish_sync_run(run.id, "succeeded", counts, None)
            .unwrap();

        let completed = database.sync_run(run.id).unwrap().unwrap();
        assert_eq!(completed.source_added, 3);
        assert_eq!(completed.source_removed, 2);
    }

    #[test]
    fn source_membership_delta_counts_added_and_removed_tracks() {
        let before = vec![1, 2, 3, 5];
        let after = vec![2, 3, 4, 6];

        assert_eq!(source_membership_delta(&before, &after), (2, 2));
    }

    #[test]
    fn warning_aggregation_deduplicates_and_ignores_blank_messages() {
        let warnings = vec![
            "provider unavailable".to_owned(),
            " provider unavailable ".to_owned(),
            "".to_owned(),
            "verification failed".to_owned(),
        ];

        assert_eq!(
            aggregate_warning_messages(&warnings).as_deref(),
            Some("provider unavailable; verification failed")
        );
    }

    #[test]
    fn reconciliation_counts_reflect_newly_present_local_file() {
        let path = TestDatabasePath::new("post-import-counts");
        let database = Database::open(path.0.clone()).unwrap();
        let account_id = database.upsert_source_account(&source_account()).unwrap();
        let collection_id = database
            .upsert_source_collection(&collection(account_id, "playlist"))
            .unwrap();
        let source_track_id = database
            .upsert_source_track(&source_track("track-imported", "USAAA0000200"))
            .unwrap();
        database
            .replace_collection_entries(collection_id, &[entry(0, source_track_id)])
            .unwrap();
        database
            .set_source_collection_tracking(collection_id, true)
            .unwrap();

        let before = run_core(&database);
        assert_eq!(before.missing, 1);
        assert_eq!(before.matched, 0);

        database
            .insert_local_file(&LocalFileWrite {
                path: "/music/imported.flac".into(),
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
                tag_year: Some(2026),
                tag_isrc: Some("USAAA0000200".into()),
                artwork_path: None,
                artwork_mime: None,
                scan_error: None,
            })
            .unwrap();

        let after = run_core(&database);
        assert_eq!(after.missing, 0);
        assert_eq!(after.matched, 1);
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
        database
            .set_source_collection_tracking(collection_id, true)
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
        let page = database
            .source_collection_page(collection_id, 0, 10)
            .unwrap()
            .unwrap();
        assert_eq!(page.entries.len(), 3);
        assert_eq!(page.entries[0].position, 0);
        assert_eq!(page.entries[1].position, 1);
        assert_eq!(page.entries[2].position, 2);
    }

    #[test]
    fn refreshed_spotify_source_materializes_tracked_missing_tracks_for_staging() {
        let path = TestDatabasePath::new("refresh-staging");
        let database = Database::open(path.0.clone()).unwrap();
        let account_id = database.upsert_source_account(&source_account()).unwrap();
        let collection_id = database
            .upsert_source_collection(&collection(account_id, "playlist"))
            .unwrap();
        let source_track_id = database
            .upsert_source_track(&source_track("track-refresh", "USAAA0000099"))
            .unwrap();
        database
            .replace_collection_entries(collection_id, &[entry(0, source_track_id)])
            .unwrap();
        database
            .set_source_collection_tracking(collection_id, true)
            .unwrap();

        assert_eq!(database.staging_items_page(0, 10).unwrap().total, 0);

        reconcile_refreshed_spotify_source(&database).unwrap();

        let staging = database.staging_items_page(0, 10).unwrap();
        assert_eq!(staging.total, 1);
        assert_eq!(staging.items[0].title, "Song");
        assert_eq!(
            database
                .persisted_track_link(source_track_id)
                .unwrap()
                .unwrap()
                .library_track_id,
            staging.items[0].library_track_id
        );
        let mirrored = database
            .list_local_playlists()
            .unwrap()
            .into_iter()
            .find(|playlist| playlist.source_collection_id == Some(collection_id))
            .expect("tracked Spotify playlist should create a local mirror");
        let detail = database.get_local_playlist(mirrored.id).unwrap();
        assert_eq!(detail.entries.len(), 1);
        assert_eq!(
            detail.entries[0].library_track_id,
            staging.items[0].library_track_id
        );
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
            .set_source_collection_tracking(first_collection, true)
            .unwrap();
        database
            .set_source_collection_tracking(second_collection, true)
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
                tag_year: Some(2026),
                tag_isrc: Some("USAAA0000002".into()),
                artwork_path: None,
                artwork_mime: None,
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
        database
            .set_source_collection_tracking(collection_id, true)
            .unwrap();
        let run = database.create_sync_run("spotify", "manual").unwrap();
        let checks = Cell::new(0usize);

        let result = run_spotify_reconciliation_core(&database, run.id, || {
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
        let first = coordinator.begin(&database, "spotify", "manual").unwrap();
        let SyncBegin::Started(lease) = first else {
            panic!("first sync should start");
        };
        let first_id = lease.run_id();
        let second = coordinator.begin(&database, "spotify", "manual").unwrap();
        assert!(matches!(second, SyncBegin::Existing(id) if id == first_id));
        drop(lease);
        assert!(matches!(
            coordinator.begin(&database, "spotify", "manual").unwrap(),
            SyncBegin::Started(_)
        ));
    }
}
