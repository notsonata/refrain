use std::{path::Path, sync::Arc};

use serde::Serialize;
use tauri::Emitter;

use crate::{
    app::AppState,
    db::{Database, DatabaseError},
    domain::{AppSettings, SourceCollectionListPage, SourceCollectionPage, SpotifySourceOverview},
    saved_albums::refresh_spotify_saved_albums,
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
    state.source_refresh.cancel()
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
