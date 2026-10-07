mod acquisition;
mod antra;
mod app;
mod commands;
mod db;
mod domain;
mod issues;
mod local_library;
mod matching;
mod monochrome;
mod normalization;
mod playlist_artwork;
mod playlist_export;
mod playlist_sync;
mod reconciliation;
mod saved_albums;
mod security;
mod sockseek;
mod source_sync;
mod spotify;
mod verification;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .setup(|application| {
            let app_data_dir = application.path().app_data_dir()?;
            let state = app::initialize(app_data_dir)?;
            application.manage(state);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_app_info,
            commands::open_external_url,
            commands::download_remote_file,
            commands::reveal_in_file_manager,
            commands::get_settings,
            commands::update_settings,
            commands::get_local_library_overview,
            commands::list_library_tracks,
            commands::list_local_files,
            commands::scan_local_library,
            commands::hash_local_file,
            commands::set_preferred_local_file,
            commands::list_local_playlists,
            commands::get_local_playlist,
            commands::create_local_playlist,
            commands::rename_local_playlist,
            commands::delete_local_playlist,
            commands::set_local_playlist_m3u_path,
            commands::add_tracks_to_local_playlist,
            commands::remove_local_playlist_entry,
            commands::move_local_playlist_entry,
            commands::sync_local_playlist,
            commands::export_playlist,
            commands::list_playlist_exports,
            commands::get_match_candidates,
            commands::confirm_match,
            commands::reject_match,
            commands::clear_match_decision,
            commands::list_issues,
            commands::get_match_review,
            commands::trash_invalid_local_file,
            commands::list_acquisition_jobs,
            commands::list_staging_items,
            commands::start_staging_track,
            commands::start_all_staging,
            commands::retry_failed_staging,
            commands::cancel_staging_track,
            commands::cancel_active_staging,
            commands::reset_acquisition_session,
            commands::resolve_staging_track,
            commands::reject_staging_candidate,
            commands::search_again_staging_track,
            commands::continue_staging_tracks,
            commands::exclude_staging_track_from_tracking,
            commands::get_soulseek_credential_status,
            commands::set_soulseek_credentials,
            commands::clear_soulseek_credentials,
            commands::get_antra_account_status,
            commands::start_antra_device_login,
            commands::poll_antra_device_login,
            commands::clear_antra_device_token,
            commands::get_antra_provider_health,
            commands::get_sockseek_provider_health,
            commands::get_monochrome_provider_health,
            reconciliation::start_sync,
            reconciliation::start_local_sync,
            reconciliation::start_spotify_sync,
            reconciliation::cancel_sync,
            reconciliation::get_sync_run,
            reconciliation::list_sync_runs,
            commands::get_spotify_auth_status,
            commands::open_external_url,
            commands::download_remote_file,
            commands::connect_spotify,
            commands::disconnect_spotify,
            commands::refresh_spotify_source,
            commands::cancel_spotify_source_refresh,
            commands::get_spotify_source_overview,
            commands::list_spotify_playlists,
            commands::list_spotify_saved_albums,
            commands::get_source_collection_page,
            commands::set_source_collection_tracking,
            commands::set_source_track_tracking,
            commands::set_source_tracks_tracking,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Refrain");
}
