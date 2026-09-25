use std::{path::Path, sync::Arc};

use serde::Serialize;
use tauri::Emitter;

use crate::{
    app::AppState,
    db::{Database, DatabaseError},
    domain::{
        AppSettings, IssuePage, LibraryTrackPage, LocalFilePage, LocalLibraryOverview, MatchResult,
        MatchReview, SourceCollectionListPage, SourceCollectionPage, SpotifySourceOverview,
    },
    issues::{get_match_review as load_match_review, list_issues as load_issues},
    local_library::{
        LOCAL_LIBRARY_SCAN_PROGRESS_EVENT, LocalLibraryError, LocalLibraryScanSummary,
        ensure_local_file_hash, scan_library,
    },
    matching::MatcherIndex,
    saved_albums::{
        cancel_spotify_saved_album_refresh, prepare_spotify_saved_album_refresh,
        refresh_spotify_saved_albums,
    },
    source_sync::{
        SOURCE_REFRESH_PROGRESS_EVENT, SourceRefreshError, SourceRefreshProgress,
        SourceRefreshSummary, refresh_spotify_source as run_spotify_source_refresh,
    },
    spotify::{SpotifyAuthError, SpotifyAuthStatus, SpotifyClient},
};

#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    name: &'static str,
    version: &'static str,
    data_dir: String,
}

#[tauri::command]
pub fn get_app_info(state: tauri::State<'_, AppState>) -> AppInfo {
    app_info_for(&state.app_data_dir)
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

    state
        .database
        .update_settings(&settings)
        .map_err(|error| command_database_error("update settings", error))
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

    tauri::async_runtime::spawn_blocking(move || {
        scan_library(&database, Path::new(&root), |progress| {
            if let Err(error) = app.emit(LOCAL_LIBRARY_SCAN_PROGRESS_EVENT, progress) {
                tracing::warn!(%error, "local library scan progress event failed");
            }
        })
    })
    .await
    .map_err(|error| {
        tracing::error!(%error, "local library scan worker failed");
        "Local library scan stopped unexpectedly.".to_owned()
    })?
    .map_err(local_library_error)
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
    state
        .database
        .set_preferred_local_file(local_file_id)
        .map_err(|error| command_database_error("set preferred local file", error))
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
    state
        .database
        .confirm_match(source_track_id, library_track_id)
        .map_err(|error| command_database_error("confirm track match", error))
}

#[tauri::command]
pub fn reject_match(
    source_track_id: i64,
    library_track_id: i64,
    reason: Option<String>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state
        .database
        .reject_match(source_track_id, library_track_id, reason.as_deref())
        .map_err(|error| command_database_error("reject track match", error))
}

#[tauri::command]
pub fn clear_match_decision(
    source_track_id: i64,
    library_track_id: Option<i64>,
    state: tauri::State<'_, AppState>,
) -> Result<(), String> {
    state
        .database
        .clear_match_decision(source_track_id, library_track_id)
        .map_err(|error| command_database_error("clear track match decision", error))
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

    tauri::async_runtime::spawn_blocking(move || {
        let _guard = guard;
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
        assert_eq!(info.version, "0.1.0");
        assert_eq!(info.data_dir, "/tmp/refrain");
    }
}
