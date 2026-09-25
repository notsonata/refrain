mod app;
mod commands;
mod db;
mod domain;
mod local_library;
mod matching;
mod normalization;
mod reconciliation;
mod saved_albums;
mod security;
mod source_sync;
mod spotify;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|application| {
            let app_data_dir = application.path().app_data_dir()?;
            let state = app::initialize(app_data_dir)?;
            application.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_info,
            commands::get_settings,
            commands::update_settings,
            commands::get_local_library_overview,
            commands::list_local_files,
            commands::scan_local_library,
            commands::hash_local_file,
            commands::set_preferred_local_file,
            commands::get_match_candidates,
            commands::confirm_match,
            commands::reject_match,
            commands::clear_match_decision,
            reconciliation::start_sync,
            reconciliation::cancel_sync,
            reconciliation::get_sync_run,
            reconciliation::list_sync_runs,
            commands::get_spotify_auth_status,
            commands::connect_spotify,
            commands::disconnect_spotify,
            commands::refresh_spotify_source,
            commands::cancel_spotify_source_refresh,
            commands::get_spotify_source_overview,
            commands::list_spotify_playlists,
            commands::list_spotify_saved_albums,
            commands::get_source_collection_page,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Refrain");
}
