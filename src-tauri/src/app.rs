use std::{error::Error, fs, path::PathBuf, sync::Arc};

use tracing_subscriber::EnvFilter;

use crate::{
    db::Database, reconciliation::SyncCoordinator, source_sync::SourceRefreshControl,
    spotify::SpotifyClient,
};

pub struct AppState {
    pub app_data_dir: PathBuf,
    pub database: Arc<Database>,
    pub spotify: Arc<SpotifyClient>,
    pub source_refresh: Arc<SourceRefreshControl>,
    pub sync: Arc<SyncCoordinator>,
}

impl AppState {
    pub fn new(
        app_data_dir: PathBuf,
        database: Arc<Database>,
        spotify: Arc<SpotifyClient>,
    ) -> Self {
        Self {
            app_data_dir,
            database,
            spotify,
            source_refresh: Arc::new(SourceRefreshControl::default()),
            sync: Arc::new(SyncCoordinator::default()),
        }
    }
}

pub fn initialize(app_data_dir: PathBuf) -> Result<AppState, Box<dyn Error>> {
    fs::create_dir_all(&app_data_dir)?;

    let log_dir = app_data_dir.join("logs");
    fs::create_dir_all(&log_dir)?;
    initialize_logging(log_dir);

    let database = Arc::new(Database::open(app_data_dir.join("refrain.sqlite3"))?);
    let spotify = Arc::new(SpotifyClient::new()?);

    tracing::info!(
        app_data_dir = %app_data_dir.display(),
        database_path = %database.path().display(),
        "application persistence initialized"
    );

    Ok(AppState::new(app_data_dir, database, spotify))
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
