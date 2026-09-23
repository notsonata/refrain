mod app;
mod commands;
mod db;
mod domain;
mod security;
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
            commands::get_spotify_auth_status,
            commands::connect_spotify,
            commands::disconnect_spotify,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Refrain");
}
