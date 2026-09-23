use std::{fs, io, path::PathBuf};

use tracing_subscriber::EnvFilter;

#[derive(Debug)]
pub struct AppState {
    pub app_data_dir: PathBuf,
}

impl AppState {
    pub fn new(app_data_dir: PathBuf) -> Self {
        Self { app_data_dir }
    }
}

pub fn initialize(app_data_dir: PathBuf) -> Result<AppState, io::Error> {
    fs::create_dir_all(&app_data_dir)?;

    let log_dir = app_data_dir.join("logs");
    fs::create_dir_all(&log_dir)?;
    initialize_logging(log_dir);

    tracing::info!(app_data_dir = %app_data_dir.display(), "application foundation initialized");

    Ok(AppState::new(app_data_dir))
}

fn initialize_logging(log_dir: PathBuf) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let file_appender = tracing_appender::rolling::daily(log_dir, "refrain.jsonl");

    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .json()
        .with_ansi(false)
        .with_writer(file_appender)
        .try_init();
}
