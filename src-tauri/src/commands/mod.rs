use std::path::Path;

use serde::Serialize;

use crate::app::AppState;

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
