use std::{fs, path::Path, sync::Arc, time::Duration};

#[cfg(any(target_os = "macos", target_os = "windows"))]
use std::process::Command;

use serde::Serialize;
use tauri::Emitter;

use crate::{
    acquisition::{AcquisitionCoordinator, AcquisitionProvider},
    antra::{AntraAccountStatus, AntraDeviceCode, AntraDeviceLoginStatus},
    app::AppState,
    db::{Database, DatabaseError},
    domain::{
        AcquisitionJob, AcquisitionJobPage, AppSettings, IssuePage, LibraryTrackPage,
        LocalFilePage, LocalLibraryOverview, LocalPlaylistDetail, LocalPlaylistSummary,
        MatchResult, MatchReview, PlaylistExport, PlaylistExportPage, ProviderHealth,
        SourceCollectionListPage, SourceCollectionPage, SpotifySourceOverview, StagingPage,
    },
    issues::{get_match_review as load_match_review, list_issues as load_issues},
    local_library::{
        LOCAL_LIBRARY_SCAN_PROGRESS_EVENT, LocalLibraryError, LocalLibraryScanSummary,
        ensure_local_file_hash, scan_library,
    },
    matching::MatcherIndex,
    normalization::trash_invalid_local_file as trash_invalid_file,
    playlist_export::{PlaylistExportCommandError, export_playlist as run_playlist_export},
    playlist_sync::{
        sync_local_playlist as run_local_playlist_sync, sync_local_playlist_after_change,
        sync_managed_local_playlists,
    },
    reconciliation::reconcile_refreshed_spotify_source,
    saved_albums::{
        cancel_spotify_saved_album_refresh, prepare_spotify_saved_album_refresh,
        refresh_spotify_saved_albums,
    },
    sockseek::SoulseekCredentialStatus,
    source_sync::{
        SOURCE_REFRESH_PROGRESS_EVENT, SourceRefreshError, SourceRefreshProgress,
        SourceRefreshSummary, refresh_spotify_source as run_spotify_source_refresh,
    },
    spotify::{SpotifyAuthError, SpotifyAuthStatus, SpotifyClient},
    verification::{ContinueStagingSummary, continue_downloaded_jobs},
};

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    name: &'static str,
    version: &'static str,
    data_dir: String,
}

fn acquire_library_write_lock(
    lock: &std::sync::RwLock<()>,
) -> Result<std::sync::RwLockWriteGuard<'_, ()>, String> {
    lock.write()
        .map_err(|_| "Library mutation lock is poisoned.".to_owned())
}

fn acquire_library_read_lock(
    lock: &std::sync::RwLock<()>,
) -> Result<std::sync::RwLockReadGuard<'_, ()>, String> {
    lock.read()
        .map_err(|_| "Library mutation lock is poisoned.".to_owned())
}

#[tauri::command]
pub fn get_app_info(state: tauri::State<'_, AppState>) -> AppInfo {
    app_info_for(&state.app_data_dir)
}

fn validated_external_url(value: &str) -> Result<url::Url, String> {
    let parsed = url::Url::parse(value).map_err(|_| "Invalid external URL.".to_owned())?;
    if parsed.scheme() != "https" {
        return Err("Only HTTPS external URLs are allowed.".to_owned());
    }
    Ok(parsed)
}

#[tauri::command]
pub fn open_external_url(url: String) -> Result<(), String> {
    let parsed = validated_external_url(&url)?;
    open::that(parsed.as_str()).map_err(|error| format!("Failed to open external link: {error}"))
}

#[tauri::command]
pub fn reveal_in_file_manager(path: String) -> Result<(), String> {
    let path = Path::new(&path);
    if !path.is_absolute() {
        return Err("File path must be absolute.".to_owned());
    }
    if !path.exists() {
        return Err("The selected file no longer exists.".to_owned());
    }

    #[cfg(target_os = "macos")]
    {
        let status = Command::new("open")
            .arg("-R")
            .arg(path)
            .status()
            .map_err(|error| format!("Failed to reveal file in Finder: {error}"))?;
        return status
            .success()
            .then_some(())
            .ok_or_else(|| "Finder could not reveal the selected file.".to_owned());
    }

    #[cfg(target_os = "windows")]
    {
        let status = Command::new("explorer.exe")
            .arg("/select,")
            .arg(path)
            .status()
            .map_err(|error| format!("Failed to reveal file in File Explorer: {error}"))?;
        return status
            .success()
            .then_some(())
            .ok_or_else(|| "File Explorer could not reveal the selected file.".to_owned());
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let directory = if path.is_dir() {
            path
        } else {
            path.parent().unwrap_or(path)
        };
        return open::that(directory)
            .map_err(|error| format!("Failed to open the containing folder: {error}"));
    }

    #[allow(unreachable_code)]
    Err("Reveal in file manager is not supported on this platform.".to_owned())
}

#[tauri::command]
pub async fn download_remote_file(url: String, destination: String) -> Result<(), String> {
    let parsed = validated_external_url(&url)?;
    tauri::async_runtime::spawn_blocking(move || {
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|error| format!("Failed to initialize download client: {error}"))?;
        let response = client
            .get(parsed)
            .send()
            .map_err(|error| format!("Failed to download album artwork: {error}"))?
            .error_for_status()
            .map_err(|error| format!("Album artwork download failed: {error}"))?;

        if let Some(content_type) = response.headers().get(reqwest::header::CONTENT_TYPE) {
            let content_type = content_type.to_str().unwrap_or_default();
            if !content_type.starts_with("image/") {
                return Err(
                    "Spotify returned a non-image response for the album artwork.".to_owned(),
                );
            }
        }

        let bytes = response
            .bytes()
            .map_err(|error| format!("Failed to read album artwork: {error}"))?;
        std::fs::write(&destination, &bytes)
            .map_err(|error| format!("Failed to save album artwork: {error}"))?;
        Ok(())
    })
    .await
    .map_err(|error| format!("Album artwork download worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
pub fn get_settings(state: tauri::State<'_, AppState>) -> Result<AppSettings, String> {
    state
        .database
        .get_settings()
        .map_err(|error| command_database_error("load settings", error))
}

#[tauri::command]
pub fn update_settings(
    settings: AppSettings,
    state: tauri::State<'_, AppState>,
) -> Result<AppSettings, String> {
    settings.validate().map_err(str::to_owned)?;

    let updated = state
        .database
        .update_settings(&settings)
        .map_err(|error| command_database_error("update settings", error))?;
    sync_managed_local_playlists(&state.database)
        .map_err(|error| command_database_error("sync managed playlists", error))?;
    Ok(updated)
}

#[tauri::command]
pub fn get_local_library_overview(
    state: tauri::State<'_, AppState>,
) -> Result<LocalLibraryOverview, String> {
    state
        .database
        .local_library_overview()
        .map_err(|error| command_database_error("load local library overview", error))
}

#[tauri::command]
pub fn list_library_tracks(
    offset: u32,
    limit: u32,
    state: tauri::State<'_, AppState>,
) -> Result<LibraryTrackPage, String> {
    state
        .database
        .library_tracks_page(offset, limit)
        .map_err(|error| command_database_error("load library tracks", error))
}

#[tauri::command]
pub fn list_local_files(
    offset: u32,
    limit: u32,
    state: tauri::State<'_, AppState>,
) -> Result<LocalFilePage, String> {
    state
        .database
        .local_files_page(offset, limit)
        .map_err(|error| command_database_error("load local files", error))
}

#[tauri::command]
pub async fn scan_local_library(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<LocalLibraryScanSummary, String> {
    let root = state
        .database
        .get_settings()
        .map_err(|error| command_database_error("load library configuration", error))?
        .library_root
        .filter(|root| !root.trim().is_empty())
        .ok_or_else(|| "Choose a local library folder in Settings before scanning.".to_owned())?;
    let database = Arc::clone(&state.database);
    let library_lock = Arc::clone(&state.library_lock);

    tauri::async_runtime::spawn_blocking(move || {
        let _library_guard = acquire_library_write_lock(&library_lock)?;
        scan_library(&database, Path::new(&root), |progress| {
            if let Err(error) = app.emit(LOCAL_LIBRARY_SCAN_PROGRESS_EVENT, progress) {
                tracing::warn!(%error, "local library scan progress event failed");
            }
        })
        .map_err(local_library_error)
    })
    .await
    .map_err(|error| {
        tracing::error!(%error, "local library scan worker failed");
        "Local library scan stopped unexpectedly.".to_owned()
    })?
}

#[tauri::command]
pub async fn hash_local_file(
    local_file_id: i64,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let database = Arc::clone(&state.database);
    tauri::async_runtime::spawn_blocking(move || ensure_local_file_hash(&database, local_file_id))
        .await
        .map_err(|error| {
            tracing::error!(%error, "local file hash worker failed");
            "Local file hashing stopped unexpectedly.".to_owned()
        })?
        .map_err(local_library_error)
}

#[tauri::command]
pub fn set_preferred_local_file(
    local_file_id: i64,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let _library_guard = acquire_library_write_lock(&state.library_lock)?;
    state
        .database
        .set_preferred_local_file(local_file_id)
        .map_err(|error| command_database_error("set preferred local file", error))
}

#[tauri::command]
pub fn list_local_playlists(
    state: tauri::State<'_, AppState>,
) -> Result<Vec<LocalPlaylistSummary>, String> {
    state
        .database
        .list_local_playlists()
        .map_err(local_playlist_database_error)
}

#[tauri::command]
pub fn get_local_playlist(
    playlist_id: i64,
    state: tauri::State<'_, AppState>,
) -> Result<LocalPlaylistDetail, String> {
    state
        .database
        .get_local_playlist(playlist_id)
        .map_err(local_playlist_database_error)
}

#[tauri::command]
pub fn create_local_playlist(
    name: String,
    state: tauri::State<'_, AppState>,
) -> Result<LocalPlaylistDetail, String> {
    let playlist = state
        .database
        .create_local_playlist(&name)
        .map_err(local_playlist_database_error)?;
    sync_local_playlist_after_change(&state.database, playlist.id)
        .map_err(local_playlist_database_error)
}

#[tauri::command]
pub fn rename_local_playlist(
    playlist_id: i64,
    name: String,
    state: tauri::State<'_, AppState>,
) -> Result<LocalPlaylistDetail, String> {
    let playlist = state
        .database
        .rename_local_playlist(playlist_id, &name)
        .map_err(local_playlist_database_error)?;
    sync_local_playlist_after_change(&state.database, playlist.id)
        .map_err(local_playlist_database_error)
}

#[tauri::command]
pub fn delete_local_playlist(
    playlist_id: i64,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state
        .database
        .delete_local_playlist(playlist_id)
        .map_err(local_playlist_database_error)
}

#[tauri::command]
pub fn set_local_playlist_m3u_path(
    playlist_id: i64,
    path: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<LocalPlaylistDetail, String> {
    state
        .database
        .set_local_playlist_m3u_path(playlist_id, path.as_deref())
        .map_err(local_playlist_database_error)
}

#[tauri::command]
pub fn add_tracks_to_local_playlist(
    playlist_id: i64,
    library_track_ids: Vec<i64>,
    state: tauri::State<'_, AppState>,
) -> Result<LocalPlaylistDetail, String> {
    let playlist = state
        .database
        .add_tracks_to_local_playlist(playlist_id, &library_track_ids)
        .map_err(local_playlist_database_error)?;
    sync_local_playlist_after_change(&state.database, playlist.id)
        .map_err(local_playlist_database_error)
}

#[tauri::command]
pub fn remove_local_playlist_entry(
    playlist_id: i64,
    entry_id: i64,
    state: tauri::State<'_, AppState>,
) -> Result<LocalPlaylistDetail, String> {
    let playlist = state
        .database
        .remove_local_playlist_entry(playlist_id, entry_id)
        .map_err(local_playlist_database_error)?;
    sync_local_playlist_after_change(&state.database, playlist.id)
        .map_err(local_playlist_database_error)
}

#[tauri::command]
pub fn move_local_playlist_entry(
    playlist_id: i64,
    entry_id: i64,
    new_position: usize,
    state: tauri::State<'_, AppState>,
) -> Result<LocalPlaylistDetail, String> {
    let playlist = state
        .database
        .move_local_playlist_entry(playlist_id, entry_id, new_position)
        .map_err(local_playlist_database_error)?;
    sync_local_playlist_after_change(&state.database, playlist.id)
        .map_err(local_playlist_database_error)
}

#[tauri::command]
pub async fn sync_local_playlist(
    playlist_id: i64,
    state: tauri::State<'_, AppState>,
) -> Result<LocalPlaylistDetail, String> {
    let database = Arc::clone(&state.database);
    let library_lock = Arc::clone(&state.library_lock);
    tauri::async_runtime::spawn_blocking(move || {
        let _library_guard = acquire_library_read_lock(&library_lock)?;
        run_local_playlist_sync(&database, playlist_id).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Playlist sync worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
pub async fn export_playlist(
    collection_id: i64,
    mode: String,
    destination: String,
    state: tauri::State<'_, AppState>,
) -> Result<PlaylistExport, PlaylistExportCommandError> {
    let database = Arc::clone(&state.database);
    let library_lock = Arc::clone(&state.library_lock);
    let library_root = state
        .database
        .get_settings()
        .map_err(|error| PlaylistExportCommandError {
            code: "EXPORT_DATABASE_ERROR".to_owned(),
            message: error.to_string(),
            unresolved_count: None,
        })?
        .library_root
        .map(std::path::PathBuf::from);
    tauri::async_runtime::spawn_blocking(move || {
        let _library_guard = library_lock
            .read()
            .map_err(|_| PlaylistExportCommandError {
                code: "LIBRARY_BUSY".to_owned(),
                message: "Library mutation lock is poisoned.".to_owned(),
                unresolved_count: None,
            })?;
        run_playlist_export(
            &database,
            library_root.as_deref(),
            collection_id,
            &mode,
            Path::new(&destination),
        )
        .map_err(PlaylistExportCommandError::from)
    })
    .await
    .map_err(|error| PlaylistExportCommandError {
        code: "EXPORT_WORKER_FAILED".to_owned(),
        message: format!("Playlist export worker stopped unexpectedly: {error}"),
        unresolved_count: None,
    })?
}

#[tauri::command]
pub fn list_playlist_exports(
    collection_id: Option<i64>,
    offset: u32,
    limit: u32,
    state: tauri::State<'_, AppState>,
) -> Result<PlaylistExportPage, String> {
    state
        .database
        .playlist_exports_page(collection_id, offset, limit)
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_match_candidates(
    source_track_id: i64,
    state: tauri::State<'_, AppState>,
) -> Result<MatchResult, String> {
    let source = state
        .database
        .source_match_track(source_track_id)
        .map_err(|error| command_database_error("load source track for matching", error))?
        .ok_or_else(|| "Source track was not found.".to_owned())?;
    let library_tracks = state
        .database
        .library_match_tracks()
        .map_err(|error| command_database_error("load library tracks for matching", error))?;
    let existing_link = state
        .database
        .persisted_track_link(source_track_id)
        .map_err(|error| command_database_error("load persisted track link", error))?;
    let rejected = state
        .database
        .rejected_library_track_ids(source_track_id)
        .map_err(|error| command_database_error("load rejected track matches", error))?;

    Ok(MatcherIndex::new(library_tracks).match_track(&source, existing_link.as_ref(), &rejected))
}

#[tauri::command]
pub fn confirm_match(
    source_track_id: i64,
    library_track_id: i64,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let _library_guard = acquire_library_write_lock(&state.library_lock)?;
    state
        .database
        .confirm_match(source_track_id, library_track_id)
        .map_err(|error| command_database_error("confirm track match", error))?;
    state
        .database
        .sync_tracked_spotify_playlist_mirrors()
        .map_err(|error| command_database_error("update tracked playlist mirrors", error))
}

#[tauri::command]
pub fn reject_match(
    source_track_id: i64,
    library_track_id: i64,
    reason: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let _library_guard = acquire_library_write_lock(&state.library_lock)?;
    state
        .database
        .reject_match(source_track_id, library_track_id, reason.as_deref())
        .map_err(|error| command_database_error("reject track match", error))?;
    state
        .database
        .sync_tracked_spotify_playlist_mirrors()
        .map_err(|error| command_database_error("update tracked playlist mirrors", error))
}

#[tauri::command]
pub fn clear_match_decision(
    source_track_id: i64,
    library_track_id: Option<i64>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let _library_guard = acquire_library_write_lock(&state.library_lock)?;
    state
        .database
        .clear_match_decision(source_track_id, library_track_id)
        .map_err(|error| command_database_error("clear track match decision", error))?;
    state
        .database
        .sync_tracked_spotify_playlist_mirrors()
        .map_err(|error| command_database_error("update tracked playlist mirrors", error))
}

#[tauri::command]
pub fn list_issues(
    offset: u32,
    limit: u32,
    state: tauri::State<'_, AppState>,
) -> Result<IssuePage, String> {
    load_issues(&state.database, offset, limit)
        .map_err(|error| command_database_error("load unresolved issues", error))
}

#[tauri::command]
pub fn get_match_review(
    source_track_id: i64,
    state: tauri::State<'_, AppState>,
) -> Result<MatchReview, String> {
    load_match_review(&state.database, source_track_id)
        .map_err(|error| command_database_error("load match review", error))?
        .ok_or_else(|| "Source track was not found.".to_owned())
}

#[tauri::command]
pub fn trash_invalid_local_file(
    local_file_id: i64,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let _library_guard = acquire_library_write_lock(&state.library_lock)?;
    let library_root = state
        .database
        .get_settings()
        .map_err(|error| command_database_error("load library configuration", error))?
        .library_root
        .filter(|root| !root.trim().is_empty())
        .ok_or_else(|| "Choose a local library folder in Settings first.".to_owned())?;

    trash_invalid_file(&state.database, Path::new(&library_root), local_file_id).map_err(|error| {
        tracing::error!(%error, local_file_id, "failed to trash invalid local file");
        error.to_string()
    })
}

#[tauri::command]
pub fn list_acquisition_jobs(
    offset: u32,
    limit: u32,
    state: tauri::State<'_, AppState>,
) -> Result<AcquisitionJobPage, String> {
    state
        .database
        .acquisition_jobs_page(offset, limit)
        .map_err(|error| command_database_error("load acquisition jobs", error))
}

#[tauri::command]
pub fn list_staging_items(
    offset: u32,
    limit: u32,
    state: tauri::State<'_, AppState>,
) -> Result<StagingPage, String> {
    let mut page = state
        .database
        .staging_items_page(offset, limit)
        .map_err(|error| command_database_error("load staging items", error))?;
    for item in &mut page.items {
        let Some(provider_job_id) = item.provider_job_id.as_deref() else {
            continue;
        };
        let progress = match item.provider.as_deref() {
            Some("monochrome") => state.monochrome.progress_for_job(provider_job_id),
            Some("antra") => state.antra.progress_for_job(provider_job_id),
            Some("sockseek") => state.sockseek.progress_for_job(provider_job_id),
            _ => None,
        };
        if let Some((transferred, total)) = progress {
            item.bytes_transferred = Some(transferred);
            item.total_bytes = Some(total);
        }
    }
    Ok(page)
}

fn acquisition_enabled(database: &Database) -> Result<(), String> {
    let settings = database
        .get_settings()
        .map_err(|error| command_database_error("load acquisition settings", error))?;
    settings.acquisition_enabled.then_some(()).ok_or_else(|| {
        "Enable acquisition in Settings before starting Staging downloads.".to_owned()
    })
}

fn acquisition_provider_by_id(
    app: &tauri::AppHandle,
    state: &AppState,
    provider_id: &str,
) -> Result<Arc<dyn AcquisitionProvider>, String> {
    match provider_id {
        "monochrome" => {
            let provider: Arc<dyn AcquisitionProvider> = state.monochrome.clone();
            Ok(provider)
        }
        "antra" => {
            let provider: Arc<dyn AcquisitionProvider> = state.antra.clone();
            Ok(provider)
        }
        "sockseek" => state
            .sockseek
            .provider(app, &state.app_data_dir)
            .map(|provider| provider as Arc<dyn AcquisitionProvider>)
            .map_err(|error| error.to_string()),
        _ => Err(format!("Unsupported acquisition provider: {provider_id}")),
    }
}

fn configured_acquisition_providers(
    app: &tauri::AppHandle,
    state: &AppState,
) -> Result<Vec<Arc<dyn AcquisitionProvider>>, String> {
    let settings = state
        .database
        .get_settings()
        .map_err(|error| command_database_error("load acquisition settings", error))?;
    let mut providers = Vec::new();
    let mut failures = Vec::new();
    for provider_id in &settings.acquisition_providers {
        match acquisition_provider_by_id(app, state, provider_id) {
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
    Ok(providers)
}

fn staging_import_root(database: &Database) -> Result<String, String> {
    database
        .get_settings()
        .map_err(|error| command_database_error("load library settings", error))?
        .library_root
        .filter(|path| !path.trim().is_empty())
        .ok_or_else(|| {
            "Choose a local library folder before downloading and importing tracks.".to_owned()
        })
}

fn import_completed_acquisitions(
    database: &Database,
    library_root: &str,
    jobs: &[AcquisitionJob],
    library_lock: &std::sync::RwLock<()>,
) -> Result<(), String> {
    let job_ids = jobs
        .iter()
        .filter(|job| job.status == "staged")
        .map(|job| job.id)
        .collect::<Vec<_>>();
    if job_ids.is_empty() {
        return Ok(());
    }

    let _library_guard = acquire_library_write_lock(library_lock)?;
    let summary = continue_downloaded_jobs(database, Path::new(library_root), &job_ids);
    if summary.failed == 0 {
        return Ok(());
    }
    Err(format!(
        "{} downloaded {} could not be verified or imported. Check Staging for details.",
        summary.failed,
        if summary.failed == 1 {
            "track"
        } else {
            "tracks"
        }
    ))
}

#[tauri::command]
pub async fn start_staging_track(
    library_track_id: i64,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    acquisition_enabled(&state.database)?;
    let library_root = staging_import_root(&state.database)?;
    let providers = configured_acquisition_providers(&app, state.inner())?;
    let database = Arc::clone(&state.database);
    let library_lock = Arc::clone(&state.library_lock);
    let app_data_dir = state.app_data_dir.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let job = AcquisitionCoordinator
            .run_library_track_chain(
                &database,
                &app_data_dir,
                &providers,
                library_track_id,
                || false,
            )
            .map_err(|error| error.to_string())?;
        import_completed_acquisitions(&database, &library_root, &[job], &library_lock)
    })
    .await
    .map_err(|error| format!("Staging worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
pub async fn start_all_staging(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    acquisition_enabled(&state.database)?;
    let library_root = staging_import_root(&state.database)?;
    let providers = configured_acquisition_providers(&app, state.inner())?;
    let database = Arc::clone(&state.database);
    let library_lock = Arc::clone(&state.library_lock);
    let app_data_dir = state.app_data_dir.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let jobs = AcquisitionCoordinator
            .run_missing_chain(&database, &app_data_dir, &providers, || false)
            .map_err(|error| error.to_string())?;
        import_completed_acquisitions(&database, &library_root, &jobs, &library_lock)
    })
    .await
    .map_err(|error| format!("Staging worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
pub async fn retry_failed_staging(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    acquisition_enabled(&state.database)?;
    let library_root = staging_import_root(&state.database)?;
    let providers = configured_acquisition_providers(&app, state.inner())?;
    let database = Arc::clone(&state.database);
    let library_lock = Arc::clone(&state.library_lock);
    let app_data_dir = state.app_data_dir.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let jobs = AcquisitionCoordinator
            .retry_failed_chain(&database, &app_data_dir, &providers, || false)
            .map_err(|error| error.to_string())?;
        import_completed_acquisitions(&database, &library_root, &jobs, &library_lock)
    })
    .await
    .map_err(|error| format!("Staging retry worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
pub fn cancel_staging_track(job_id: i64, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let Some(job) = state
        .database
        .acquisition_job(job_id)
        .map_err(|error| command_database_error("load acquisition job", error))?
    else {
        return Ok(());
    };
    if let Some(provider_job_id) = job.provider_job_id.as_deref() {
        match job.provider.as_str() {
            "monochrome" => state
                .monochrome
                .cancel_active_job(provider_job_id)
                .map_err(|error| error.to_string())?,
            "antra" => state
                .antra
                .cancel_active_job(provider_job_id)
                .map_err(|error| error.to_string())?,
            "sockseek" => state
                .sockseek
                .cancel_active_job(provider_job_id)
                .map_err(|error| error.to_string())?,
            _ => {}
        }
    }
    state
        .database
        .finish_acquisition_job(job_id, "cancelled", None, None)
        .map_err(|error| command_database_error("cancel acquisition job", error))
}

#[tauri::command]
pub fn cancel_active_staging(state: tauri::State<'_, AppState>) -> Result<(), String> {
    let jobs = state
        .database
        .active_acquisition_jobs()
        .map_err(|error| command_database_error("load active acquisition jobs", error))?;
    for job in jobs {
        if let Some(provider_job_id) = job.provider_job_id.as_deref() {
            match job.provider.as_str() {
                "monochrome" => state
                    .monochrome
                    .cancel_active_job(provider_job_id)
                    .map_err(|error| error.to_string())?,
                "antra" => state
                    .antra
                    .cancel_active_job(provider_job_id)
                    .map_err(|error| error.to_string())?,
                "sockseek" => state
                    .sockseek
                    .cancel_active_job(provider_job_id)
                    .map_err(|error| error.to_string())?,
                _ => {}
            }
        }
        state
            .database
            .finish_acquisition_job(job.id, "cancelled", None, None)
            .map_err(|error| command_database_error("cancel acquisition job", error))?;
    }
    Ok(())
}

#[tauri::command]
pub fn reset_acquisition_session(state: tauri::State<'_, AppState>) -> Result<usize, String> {
    let active_jobs = state
        .database
        .active_acquisition_jobs()
        .map_err(|error| command_database_error("load active acquisition jobs", error))?;

    for job in active_jobs {
        let Some(provider_job_id) = job.provider_job_id.as_deref() else {
            continue;
        };
        match job.provider.as_str() {
            "monochrome" => {
                if let Err(error) = state.monochrome.cancel_active_job(provider_job_id) {
                    tracing::warn!(
                        job_id = job.id,
                        provider_job_id,
                        %error,
                        "could not cancel Monochrome job while resetting acquisition session"
                    );
                }
            }
            "antra" => {
                if let Err(error) = state.antra.cancel_active_job(provider_job_id) {
                    tracing::warn!(
                        job_id = job.id,
                        provider_job_id,
                        %error,
                        "could not cancel Antra job while resetting acquisition session"
                    );
                }
            }
            "sockseek" => {
                if let Err(error) = state.sockseek.cancel_active_job(provider_job_id) {
                    tracing::warn!(
                        job_id = job.id,
                        provider_job_id,
                        %error,
                        "could not cancel Sockseek job while resetting acquisition session"
                    );
                }
            }
            _ => {}
        }
    }

    let cleared = state
        .database
        .clear_acquisition_jobs()
        .map_err(|error| command_database_error("reset acquisition session", error))?;
    state.monochrome.reset_session_state();

    let staging_root = state.app_data_dir.join("runtime").join("acquisition");
    if let Err(error) = fs::remove_dir_all(&staging_root)
        && error.kind() != std::io::ErrorKind::NotFound
    {
        tracing::warn!(
            path = %staging_root.display(),
            %error,
            "could not remove abandoned acquisition staging directory"
        );
    }

    tracing::info!(cleared, "acquisition session reset");
    Ok(cleared)
}

#[tauri::command]
pub async fn resolve_staging_track(
    job_id: i64,
    provider: String,
    provider_token: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    acquisition_enabled(&state.database)?;
    let library_root = staging_import_root(&state.database)?;
    let provider = acquisition_provider_by_id(&app, state.inner(), &provider)?;
    let database = Arc::clone(&state.database);
    let library_lock = Arc::clone(&state.library_lock);
    let app_data_dir = state.app_data_dir.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let job = AcquisitionCoordinator
            .run_selected_candidate(
                &database,
                &app_data_dir,
                provider.as_ref(),
                job_id,
                &provider_token,
                || false,
            )
            .map_err(|error| error.to_string())?;
        import_completed_acquisitions(&database, &library_root, &[job], &library_lock)
    })
    .await
    .map_err(|error| format!("Staging resolution worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
pub fn reject_staging_candidate(
    job_id: i64,
    provider: String,
    provider_token: String,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let candidates = state
        .database
        .reject_acquisition_candidate(job_id, &provider, &provider_token)
        .map_err(|error| command_database_error("reject acquisition candidate", error))?;
    if candidates.is_empty() {
        state
            .database
            .finish_acquisition_job(
                job_id,
                "failed",
                Some("noCandidates"),
                Some("All acquisition candidates were rejected. Search again to retry."),
            )
            .map_err(|error| command_database_error("finish acquisition resolution", error))?;
    }
    Ok(())
}

#[tauri::command]
pub async fn search_again_staging_track(
    job_id: i64,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    acquisition_enabled(&state.database)?;
    let library_root = staging_import_root(&state.database)?;
    let library_track_id = state
        .database
        .acquisition_job(job_id)
        .map_err(|error| command_database_error("load acquisition job", error))?
        .ok_or_else(|| "Acquisition job was not found.".to_owned())?
        .library_track_id;
    tracing::info!(job_id, library_track_id, "staging track retry started");
    let providers = configured_acquisition_providers(&app, state.inner())?;
    let database = Arc::clone(&state.database);
    let library_lock = Arc::clone(&state.library_lock);
    let app_data_dir = state.app_data_dir.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let job = AcquisitionCoordinator
            .run_library_track_chain(
                &database,
                &app_data_dir,
                &providers,
                library_track_id,
                || false,
            )
            .map_err(|error| error.to_string())?;
        let resulting_job_id = job.id;
        let resulting_status = job.status.clone();
        let resulting_stage = job.stage.clone().unwrap_or_default();
        let resulting_provider = job.provider.clone();
        let result = import_completed_acquisitions(&database, &library_root, &[job], &library_lock);
        tracing::info!(
            job_id = resulting_job_id,
            library_track_id,
            provider = %resulting_provider,
            status = %resulting_status,
            stage = %resulting_stage,
            imported = result.is_ok(),
            "staging track retry finished"
        );
        result
    })
    .await
    .map_err(|error| format!("Staging search worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
pub async fn continue_staging_tracks(
    job_ids: Vec<i64>,
    state: tauri::State<'_, AppState>,
) -> Result<ContinueStagingSummary, String> {
    if job_ids.is_empty() {
        return Ok(ContinueStagingSummary::default());
    }
    let settings = state
        .database
        .get_settings()
        .map_err(|error| command_database_error("load library settings", error))?;
    let library_root = settings
        .library_root
        .filter(|path| !path.trim().is_empty())
        .ok_or_else(|| {
            "Choose a local library folder before importing staged tracks.".to_owned()
        })?;
    let database = Arc::clone(&state.database);
    let library_lock = Arc::clone(&state.library_lock);
    tauri::async_runtime::spawn_blocking(move || {
        let _library_guard = acquire_library_write_lock(&library_lock)?;
        Ok::<_, String>(continue_downloaded_jobs(
            &database,
            Path::new(&library_root),
            &job_ids,
        ))
    })
    .await
    .map_err(|error| format!("Staging import worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
pub fn exclude_staging_track_from_tracking(
    library_track_id: i64,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let _library_guard = acquire_library_write_lock(&state.library_lock)?;
    state
        .database
        .exclude_library_track_from_tracking(library_track_id)
        .map_err(|error| command_database_error("exclude staging track from tracking", error))?;
    reconcile_refreshed_spotify_source(&state.database)
        .map_err(|error| command_database_error("reconcile Spotify tracking", error))?;
    Ok(())
}

#[tauri::command]
pub fn get_soulseek_credential_status(
    state: tauri::State<'_, AppState>,
) -> Result<SoulseekCredentialStatus, String> {
    state.sockseek.credential_status().map_err(|error| {
        tracing::error!(%error, "load Soulseek credential status failed");
        error.to_string()
    })
}

#[tauri::command]
pub fn set_soulseek_credentials(
    username: String,
    password: String,
    state: tauri::State<'_, AppState>,
) -> Result<SoulseekCredentialStatus, String> {
    state
        .sockseek
        .set_credentials(&username, &password)
        .map_err(|error| error.to_string())?;
    state
        .sockseek
        .credential_status()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn clear_soulseek_credentials(
    state: tauri::State<'_, AppState>,
) -> Result<SoulseekCredentialStatus, String> {
    state
        .sockseek
        .clear_credentials()
        .map_err(|error| error.to_string())?;
    state
        .sockseek
        .credential_status()
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn get_antra_account_status(
    state: tauri::State<'_, AppState>,
) -> Result<AntraAccountStatus, String> {
    state.antra.account_status().map_err(|error| {
        tracing::error!(%error, "load Antra account status failed");
        error.to_string()
    })
}

#[tauri::command]
pub async fn start_antra_device_login(
    state: tauri::State<'_, AppState>,
) -> Result<AntraDeviceCode, String> {
    let provider = Arc::clone(&state.antra);
    tauri::async_runtime::spawn_blocking(move || {
        provider
            .start_device_login()
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Antra sign-in worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
pub async fn poll_antra_device_login(
    device_code: String,
    state: tauri::State<'_, AppState>,
) -> Result<AntraDeviceLoginStatus, String> {
    let provider = Arc::clone(&state.antra);
    tauri::async_runtime::spawn_blocking(move || {
        provider
            .poll_device_login(&device_code)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Antra sign-in worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
pub fn clear_antra_device_token(
    state: tauri::State<'_, AppState>,
) -> Result<AntraAccountStatus, String> {
    state.antra.sign_out().map_err(|error| error.to_string())
}

#[tauri::command]
pub async fn get_monochrome_provider_health(
    state: tauri::State<'_, AppState>,
) -> Result<ProviderHealth, String> {
    let provider = Arc::clone(&state.monochrome);
    tauri::async_runtime::spawn_blocking(move || {
        provider.health().map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Monochrome health worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
pub async fn get_antra_provider_health(
    state: tauri::State<'_, AppState>,
) -> Result<ProviderHealth, String> {
    let provider = Arc::clone(&state.antra);
    tauri::async_runtime::spawn_blocking(move || {
        provider.health().map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Antra health worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
pub async fn get_sockseek_provider_health(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<ProviderHealth, String> {
    let manager = Arc::clone(&state.sockseek);
    let app_data_dir = state.app_data_dir.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let provider = manager
            .provider(&app, &app_data_dir)
            .map_err(|error| error.to_string())?;
        provider.health().map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Sockseek health worker stopped unexpectedly: {error}"))?
}

#[tauri::command]
pub fn get_spotify_auth_status(
    state: tauri::State<'_, AppState>,
) -> Result<SpotifyAuthStatus, SpotifyAuthError> {
    let client_id = spotify_client_id(&state.database)?;
    state.spotify.status(client_id)
}

#[tauri::command]
pub async fn connect_spotify(
    client_id: String,
    state: tauri::State<'_, AppState>,
) -> Result<SpotifyAuthStatus, SpotifyAuthError> {
    let database = Arc::clone(&state.database);
    let spotify = Arc::clone(&state.spotify);
    let client_id = SpotifyClient::normalize_client_id(&client_id)?;
    persist_spotify_client_id(&database, &client_id)?;

    let worker = Arc::clone(&spotify);
    let connection_client_id = client_id.clone();
    tauri::async_runtime::spawn_blocking(move || worker.connect(&connection_client_id))
        .await
        .map_err(|error| {
            tracing::error!(%error, "Spotify authentication worker failed");
            SpotifyAuthError::new(
                "authenticationWorkerFailed",
                "Spotify authentication stopped unexpectedly.",
            )
        })??;

    spotify.status(Some(client_id))
}

#[tauri::command]
pub async fn disconnect_spotify(
    state: tauri::State<'_, AppState>,
) -> Result<SpotifyAuthStatus, SpotifyAuthError> {
    let database = Arc::clone(&state.database);
    let spotify = Arc::clone(&state.spotify);
    let client_id = spotify_client_id(&database)?;
    let worker = Arc::clone(&spotify);
    tauri::async_runtime::spawn_blocking(move || worker.disconnect())
        .await
        .map_err(|error| {
            tracing::error!(%error, "Spotify disconnect worker failed");
            SpotifyAuthError::new(
                "authenticationWorkerFailed",
                "Spotify disconnect stopped unexpectedly.",
            )
        })??;

    spotify.status(client_id)
}

#[tauri::command]
pub async fn refresh_spotify_source(
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<SourceRefreshSummary, SourceRefreshError> {
    let client_id = state
        .database
        .get_settings()
        .map_err(|error| source_refresh_database_error("load Spotify configuration", error))?
        .spotify_client_id
        .ok_or_else(|| {
            SourceRefreshError::new(
                "spotifyNotConfigured",
                "Enter and connect a Spotify Client ID before refreshing source data.",
            )
        })?;
    let guard = state.source_refresh.begin()?;
    prepare_spotify_saved_album_refresh();
    let database = Arc::clone(&state.database);
    let spotify = Arc::clone(&state.spotify);
    let control = Arc::clone(&state.source_refresh);
    let library_lock = Arc::clone(&state.library_lock);

    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
        let _library_guard = acquire_library_write_lock(&library_lock)
            .map_err(|message| SourceRefreshError::new("libraryBusy", message))?;
        let summary = run_spotify_source_refresh(
            &database,
            Arc::clone(&spotify),
            &client_id,
            &control,
            |progress| {
                if let Err(error) = app.emit(SOURCE_REFRESH_PROGRESS_EVENT, progress) {
                    tracing::warn!(%error, "Spotify source refresh progress event failed");
                }
            },
        )?;

        if let Err(error) = app.emit(
            SOURCE_REFRESH_PROGRESS_EVENT,
            SourceRefreshProgress {
                phase: "savedAlbums".into(),
                completed: 0,
                total: None,
                message: "Refreshing Saved Albums".into(),
            },
        ) {
            tracing::warn!(%error, "Spotify saved album progress event failed");
        }
        let saved_albums = refresh_spotify_saved_albums(&database, spotify, &client_id)?;
        reconcile_refreshed_spotify_source(&database).map_err(|error| {
            source_refresh_database_error("reconcile refreshed Spotify tracks", error)
        })?;
        if let Err(error) = app.emit(
            SOURCE_REFRESH_PROGRESS_EVENT,
            SourceRefreshProgress {
                phase: "complete".into(),
                completed: saved_albums,
                total: Some(saved_albums),
                message: format!("Spotify source refresh complete · {saved_albums} saved albums"),
            },
        ) {
            tracing::warn!(%error, "Spotify saved album completion event failed");
        }

        Ok(summary)
    })
    .await
    .map_err(|error| {
        tracing::error!(%error, "Spotify source refresh worker failed");
        SourceRefreshError::new(
            "refreshWorkerFailed",
            "Spotify source refresh stopped unexpectedly.",
        )
    })?
}

#[tauri::command]
pub fn cancel_spotify_source_refresh(state: tauri::State<'_, AppState>) -> bool {
    let cancelled = state.source_refresh.cancel();
    cancel_spotify_saved_album_refresh();
    cancelled
}

#[tauri::command]
pub fn get_spotify_source_overview(
    state: tauri::State<'_, AppState>,
) -> Result<SpotifySourceOverview, String> {
    state
        .database
        .spotify_source_overview()
        .map_err(|error| command_database_error("load Spotify source overview", error))
}

#[tauri::command]
pub fn list_spotify_playlists(
    offset: u32,
    limit: u32,
    state: tauri::State<'_, AppState>,
) -> Result<SourceCollectionListPage, String> {
    state
        .database
        .spotify_playlists_page(offset, limit)
        .map_err(|error| command_database_error("load Spotify playlists", error))
}

#[tauri::command]
pub fn list_spotify_saved_albums(
    offset: u32,
    limit: u32,
    state: tauri::State<'_, AppState>,
) -> Result<SourceCollectionListPage, String> {
    state
        .database
        .spotify_saved_albums_page(offset, limit)
        .map_err(|error| command_database_error("load Spotify saved albums", error))
}

#[tauri::command]
pub fn get_source_collection_page(
    collection_id: i64,
    offset: u32,
    limit: u32,
    state: tauri::State<'_, AppState>,
) -> Result<SourceCollectionPage, String> {
    state
        .database
        .source_collection_page(collection_id, offset, limit)
        .map_err(|error| command_database_error("load Spotify collection", error))?
        .ok_or_else(|| "Spotify collection was not found.".to_owned())
}

#[tauri::command]
pub fn set_source_collection_tracking(
    collection_id: i64,
    included: bool,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let _library_guard = acquire_library_write_lock(&state.library_lock)?;
    state
        .database
        .set_source_collection_tracking(collection_id, included)
        .map_err(|error| command_database_error("update Spotify collection tracking", error))?;
    reconcile_refreshed_spotify_source(&state.database)
        .map_err(|error| command_database_error("reconcile Spotify tracking", error))?;
    Ok(())
}

#[tauri::command]
pub fn set_source_track_tracking(
    collection_id: i64,
    source_track_id: i64,
    included: Option<bool>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let _library_guard = acquire_library_write_lock(&state.library_lock)?;
    state
        .database
        .set_source_track_tracking(collection_id, source_track_id, included)
        .map_err(|error| command_database_error("update Spotify track tracking", error))?;
    reconcile_refreshed_spotify_source(&state.database)
        .map_err(|error| command_database_error("reconcile Spotify tracking", error))?;
    Ok(())
}

#[tauri::command]
pub fn set_source_tracks_tracking(
    collection_id: i64,
    source_track_ids: Vec<i64>,
    included: Option<bool>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    let _library_guard = acquire_library_write_lock(&state.library_lock)?;
    state
        .database
        .set_source_tracks_tracking(collection_id, &source_track_ids, included)
        .map_err(|error| command_database_error("update Spotify track tracking", error))?;
    reconcile_refreshed_spotify_source(&state.database)
        .map_err(|error| command_database_error("reconcile Spotify tracking", error))?;
    Ok(())
}

fn spotify_client_id(database: &Database) -> Result<Option<String>, SpotifyAuthError> {
    database
        .get_settings()
        .map(|settings| settings.spotify_client_id)
        .map_err(|error| spotify_database_error("load Spotify configuration", error))
}

fn persist_spotify_client_id(database: &Database, client_id: &str) -> Result<(), SpotifyAuthError> {
    let mut settings = database
        .get_settings()
        .map_err(|error| spotify_database_error("load Spotify configuration", error))?;
    settings.spotify_client_id = Some(client_id.to_owned());
    database
        .update_settings(&settings)
        .map_err(|error| spotify_database_error("save Spotify configuration", error))?;
    Ok(())
}

fn spotify_database_error(operation: &str, error: DatabaseError) -> SpotifyAuthError {
    tracing::error!(%error, operation, "Spotify persistence command failed");
    SpotifyAuthError::new("persistenceFailed", format!("Failed to {operation}."))
}

fn source_refresh_database_error(operation: &str, error: DatabaseError) -> SourceRefreshError {
    tracing::error!(%error, operation, "Spotify source refresh persistence command failed");
    SourceRefreshError::new("persistenceFailed", format!("Failed to {operation}."))
}

fn local_library_error(error: LocalLibraryError) -> String {
    tracing::error!(%error, "local library command failed");
    error.to_string()
}

fn command_database_error(operation: &str, error: DatabaseError) -> String {
    tracing::error!(%error, operation, "database command failed");
    format!("failed to {operation}")
}

fn local_playlist_database_error(error: DatabaseError) -> String {
    tracing::error!(%error, "local playlist database command failed");
    error.to_string()
}

fn app_info_for(data_dir: &Path) -> AppInfo {
    AppInfo {
        name: "Refrain",
        version: env!("CARGO_PKG_VERSION"),
        data_dir: data_dir.to_string_lossy().into_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_info_uses_expected_metadata() {
        let info = app_info_for(Path::new("/tmp/refrain"));

        assert_eq!(info.name, "Refrain");
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(info.data_dir, "/tmp/refrain");
    }
}
