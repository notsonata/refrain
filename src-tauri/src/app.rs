use std::{
    error::Error,
    fs,
    path::PathBuf,
    sync::{Arc, Mutex, RwLock, atomic::AtomicBool},
};

use tracing_subscriber::EnvFilter;

use crate::{
    antra::AntraProvider, db::Database, monochrome::MonochromeProvider,
    playlist_sync::sync_managed_local_playlists, reconciliation::SyncCoordinator,
    reconciliation::SyncPipelineLock, sockseek::SockseekManager,
    source_sync::SourceRefreshControl, spotify::SpotifyClient, sync_scheduler::SyncScheduler,
};

pub struct AppState {
    pub app_data_dir: PathBuf,
    pub database: Arc<Database>,
    pub spotify: Arc<SpotifyClient>,
    pub source_refresh: Arc<SourceRefreshControl>,
    pub sync: Arc<SyncCoordinator>,
    pub sync_pipeline: Arc<SyncPipelineLock>,
    pub sync_scheduler: SyncScheduler,
    pub library_lock: Arc<RwLock<()>>,
    pub mirror_lock: Arc<Mutex<()>>,
    pub active_mirror: Arc<Mutex<Option<ActiveMirror>>>,
    pub monochrome: Arc<MonochromeProvider>,
    pub antra: Arc<AntraProvider>,
    pub sockseek: Arc<SockseekManager>,
}

pub struct ActiveMirror {
    pub target_id: i64,
    pub cancelled: Arc<AtomicBool>,
}

impl AppState {
    pub fn new(
        app_data_dir: PathBuf,
        database: Arc<Database>,
        spotify: Arc<SpotifyClient>,
        sync_scheduler: SyncScheduler,
    ) -> Result<Self, crate::acquisition::ProviderError> {
        Ok(Self {
            app_data_dir,
            database,
            spotify,
            source_refresh: Arc::new(SourceRefreshControl::default()),
            sync: Arc::new(SyncCoordinator::default()),
            sync_pipeline: Arc::new(SyncPipelineLock::default()),
            sync_scheduler,
            library_lock: Arc::new(RwLock::new(())),
            mirror_lock: Arc::new(Mutex::new(())),
            active_mirror: Arc::new(Mutex::new(None)),
            monochrome: Arc::new(MonochromeProvider::new()?),
            antra: Arc::new(AntraProvider::new()?),
            sockseek: Arc::new(SockseekManager::default()),
        })
    }
}

pub fn initialize(app_data_dir: PathBuf) -> Result<AppState, Box<dyn Error>> {
    fs::create_dir_all(&app_data_dir)?;

    let log_dir = app_data_dir.join("logs");
    fs::create_dir_all(&log_dir)?;
    initialize_logging(log_dir);

    let database = Arc::new(Database::open(app_data_dir.join("refrain.sqlite3"))?);
    sync_managed_local_playlists(&database)?;
    let recovered_acquisitions = database.recover_interrupted_acquisition_jobs()?;
    let spotify = Arc::new(SpotifyClient::new()?);

    tracing::info!(
        app_data_dir = %app_data_dir.display(),
        database_path = %database.path().display(),
        recovered_acquisitions,
        "application persistence initialized"
    );

    let sync_scheduler = SyncScheduler::new(&database.get_settings()?);
    Ok(AppState::new(app_data_dir, database, spotify, sync_scheduler)?)
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
