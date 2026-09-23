use std::{path::Path, sync::Arc};

use serde::Serialize;

use crate::{
    app::AppState,
    db::{Database, DatabaseError},
    domain::AppSettings,
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

fn spotify_client_id(database: &Database) -> Result<Option<String>, SpotifyAuthError> {
    database
        .get_settings()
        .map(|settings| settings.spotify_client_id)
        .map_err(|error| spotify_database_error("load Spotify configuration", error))
}

fn persist_spotify_client_id(
    database: &Database,
    client_id: &str,
) -> Result<(), SpotifyAuthError> {
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
    SpotifyAuthError::new(
        "persistenceFailed",
        format!("Failed to {operation}."),
    )
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
