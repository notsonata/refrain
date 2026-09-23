use std::path::Path;

use serde::Serialize;

use crate::{app::AppState, db::DatabaseError, domain::AppSettings};

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
