// Milestone 12 persistence is exercised by the acquisition boundary before production wiring.
#[allow(dead_code)]
mod acquisition;
mod issues;
mod local_library;
mod matching;
mod migrations;
mod normalization;
mod reconciliation;
mod saved_albums;
mod settings;
mod source;
mod source_browse;
mod source_refresh;
mod source_tracking;

use std::{
    error::Error,
    fmt, fs,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use rusqlite::Connection;

use crate::domain::{AppSettings, CollectionEntry, SourceAccount, SourceCollection, SourceTrack};

pub(crate) use local_library::LocalFileWrite;
use migrations::MIGRATIONS;
pub(crate) use normalization::NormalizationCandidate;

#[derive(Debug)]
pub struct Database {
    path: PathBuf,
    connection: Mutex<Connection>,
}

impl Database {
    pub fn open(path: PathBuf) -> Result<Self, DatabaseError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let mut connection = Connection::open(&path)?;
        configure_connection(&connection)?;
        MIGRATIONS.to_latest(&mut connection)?;
        settings::ensure_default(&connection)?;

        Ok(Self {
            path,
            connection: Mutex::new(connection),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn get_settings(&self) -> Result<AppSettings, DatabaseError> {
        self.with_connection(settings::get)
    }

    pub fn update_settings(&self, value: &AppSettings) -> Result<AppSettings, DatabaseError> {
        self.with_connection(|connection| settings::update(connection, value))
    }

    pub fn upsert_source_account(&self, account: &SourceAccount) -> Result<i64, DatabaseError> {
        self.with_connection(|connection| source::upsert_account(connection, account))
    }

    pub fn mark_source_account_synced(
        &self,
        account_id: i64,
        synced_at: i64,
    ) -> Result<(), DatabaseError> {
        self.with_connection(|connection| {
            source::mark_account_synced(connection, account_id, synced_at)
        })
    }

    pub fn upsert_source_collection(
        &self,
        collection: &SourceCollection,
    ) -> Result<i64, DatabaseError> {
        self.with_connection(|connection| source::upsert_collection(connection, collection))
    }

    #[allow(dead_code)]
    pub fn upsert_source_track(&self, track: &SourceTrack) -> Result<i64, DatabaseError> {
        self.with_connection(|connection| source::upsert_track(connection, track))
    }

    #[allow(dead_code)]
    pub fn replace_collection_entries(
        &self,
        collection_id: i64,
        entries: &[CollectionEntry],
    ) -> Result<(), DatabaseError> {
        let mut connection = self.lock_connection()?;
        source::replace_collection_entries(&mut connection, collection_id, entries)?;
        Ok(())
    }

    fn with_connection<T>(
        &self,
        operation: impl FnOnce(&Connection) -> Result<T, rusqlite::Error>,
    ) -> Result<T, DatabaseError> {
        let connection = self.lock_connection()?;
        operation(&connection).map_err(DatabaseError::from)
    }

    fn lock_connection(&self) -> Result<std::sync::MutexGuard<'_, Connection>, DatabaseError> {
        self.connection
            .lock()
            .map_err(|_| DatabaseError::LockPoisoned)
    }
}

fn configure_connection(connection: &Connection) -> Result<(), rusqlite::Error> {
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    Ok(())
}

pub(crate) fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock must be after the Unix epoch")
        .as_millis() as i64
}

#[derive(Debug)]
pub enum DatabaseError {
    Io(std::io::Error),
    Sqlite(rusqlite::Error),
    Migration(rusqlite_migration::Error),
    LockPoisoned,
    InvalidState(String),
}

impl fmt::Display for DatabaseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "database filesystem error: {error}"),
            Self::Sqlite(error) => write!(formatter, "sqlite error: {error}"),
            Self::Migration(error) => write!(formatter, "database migration error: {error}"),
            Self::LockPoisoned => formatter.write_str("database connection lock is poisoned"),
            Self::InvalidState(message) => formatter.write_str(message),
        }
    }
}

impl Error for DatabaseError {}

impl From<std::io::Error> for DatabaseError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<rusqlite::Error> for DatabaseError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

impl From<rusqlite_migration::Error> for DatabaseError {
    fn from(error: rusqlite_migration::Error) -> Self {
        Self::Migration(error)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    use rusqlite::params;

    use super::*;

    static NEXT_DATABASE_ID: AtomicU64 = AtomicU64::new(0);

    struct TestDatabasePath {
        path: PathBuf,
    }

    impl TestDatabasePath {
        fn new(name: &str) -> Self {
            let id = NEXT_DATABASE_ID.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "refrain-{name}-{}-{id}.sqlite3",
                std::process::id()
            ));
            Self { path }
        }
    }

    impl Drop for TestDatabasePath {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
            let _ = fs::remove_file(self.path.with_extension("sqlite3-wal"));
            let _ = fs::remove_file(self.path.with_extension("sqlite3-shm"));
        }
    }

    fn sample_account() -> SourceAccount {
        SourceAccount {
            provider: "spotify".into(),
            provider_account_id: "account-1".into(),
            display_name: Some("Listener".into()),
            image_url: None,
            client_id: "client-id".into(),
        }
    }

    fn sample_collection(account_id: i64) -> SourceCollection {
        SourceCollection {
            source_account_id: account_id,
            provider_collection_id: "playlist-1".into(),
            kind: "playlist".into(),
            name: "Playlist".into(),
            snapshot_id: Some("snapshot-1".into()),
            owner_provider_id: Some("account-1".into()),
            is_accessible: true,
            access_issue: None,
            image_url: None,
            external_url: None,
            album_metadata: None,
        }
    }

    fn sample_track(provider_track_id: &str) -> SourceTrack {
        SourceTrack {
            provider: "spotify".into(),
            provider_track_id: provider_track_id.into(),
            uri: Some(format!("spotify:track:{provider_track_id}")),
            isrc: Some("USABC1234567".into()),
            title: "Track".into(),
            normalized_title: "track".into(),
            artists_json: "[\"Artist\"]".into(),
            normalized_artists: "artist".into(),
            album: Some("Album".into()),
            normalized_album: Some("album".into()),
            duration_ms: Some(180_000),
            disc_number: Some(1),
            track_number: Some(1),
            release_year: Some(2026),
            explicit: Some(false),
            version_kind: None,
            version_detail: None,
            image_url: None,
            external_url: None,
        }
    }

    #[test]
    fn empty_database_migrates_and_configures_sqlite() {
        let test_path = TestDatabasePath::new("migrations");
        let database = Database::open(test_path.path.clone()).expect("database should open");
        let connection = database.lock_connection().expect("database should lock");

        let foreign_keys: i64 = connection
            .pragma_query_value(None, "foreign_keys", |row| row.get(0))
            .expect("foreign key pragma should be readable");
        let journal_mode: String = connection
            .pragma_query_value(None, "journal_mode", |row| row.get(0))
            .expect("journal mode pragma should be readable");
        let user_version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .expect("user version pragma should be readable");

        assert_eq!(foreign_keys, 1);
        assert_eq!(journal_mode.to_ascii_lowercase(), "wal");
        assert_eq!(user_version, 6);

        for table in [
            "library_tracks",
            "local_files",
            "track_links",
            "track_rejections",
            "sync_runs",
            "acquisition_jobs",
        ] {
            let exists: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    [table],
                    |row| row.get(0),
                )
                .expect("library table lookup should succeed");
            assert_eq!(exists, 1, "{table} should exist after migration");
        }
        assert_eq!(database.path(), test_path.path.as_path());
    }

    #[test]
    fn migrations_are_repeatable_and_settings_persist() {
        let test_path = TestDatabasePath::new("reopen");

        {
            let database = Database::open(test_path.path.clone()).expect("database should open");
            let updated = AppSettings {
                sync_on_startup: true,
                sync_interval_minutes: Some(30),
                spotify_client_id: Some("spotify-client".into()),
                ..AppSettings::default()
            };
            database
                .update_settings(&updated)
                .expect("settings should update");
        }

        let reopened = Database::open(test_path.path.clone()).expect("database should reopen");
        let settings = reopened.get_settings().expect("settings should load");

        assert!(settings.sync_on_startup);
        assert_eq!(settings.sync_interval_minutes, Some(30));
        assert_eq!(
            settings.spotify_client_id.as_deref(),
            Some("spotify-client")
        );
    }

    #[test]
    fn foreign_keys_and_unique_constraints_are_enforced() {
        let test_path = TestDatabasePath::new("constraints");
        let database = Database::open(test_path.path.clone()).expect("database should open");
        let connection = database.lock_connection().expect("database should lock");
        let now = now_ms();

        let foreign_key_error = connection.execute(
            "INSERT INTO source_collections (
                source_account_id,
                provider_collection_id,
                kind,
                name,
                created_at,
                updated_at
             ) VALUES (9999, 'playlist', 'playlist', 'Playlist', ?1, ?1)",
            [now],
        );
        assert!(foreign_key_error.is_err());

        connection
            .execute(
                "INSERT INTO source_accounts (
                    provider,
                    provider_account_id,
                    client_id,
                    created_at,
                    updated_at
                 ) VALUES ('spotify', 'account', 'client', ?1, ?1)",
                [now],
            )
            .expect("first source account should insert");

        let unique_error = connection.execute(
            "INSERT INTO source_accounts (
                provider,
                provider_account_id,
                client_id,
                created_at,
                updated_at
             ) VALUES ('spotify', 'account', 'other-client', ?1, ?1)",
            [now],
        );
        assert!(unique_error.is_err());
    }

    #[test]
    fn source_upserts_preserve_row_identity() {
        let test_path = TestDatabasePath::new("upsert");
        let database = Database::open(test_path.path.clone()).expect("database should open");
        let first_id = database
            .upsert_source_account(&sample_account())
            .expect("source account should save");

        let mut updated = sample_account();
        updated.display_name = Some("Updated Listener".into());
        let second_id = database
            .upsert_source_account(&updated)
            .expect("source account should update");

        assert_eq!(first_id, second_id);
    }

    #[test]
    fn collection_replacement_is_atomic() {
        let test_path = TestDatabasePath::new("atomic-replace");
        let database = Database::open(test_path.path.clone()).expect("database should open");
        let account_id = database
            .upsert_source_account(&sample_account())
            .expect("source account should save");
        let collection_id = database
            .upsert_source_collection(&sample_collection(account_id))
            .expect("collection should save");
        let track_id = database
            .upsert_source_track(&sample_track("track-1"))
            .expect("track should save");

        let original = vec![CollectionEntry {
            position: 0,
            source_track_id: Some(track_id),
            provider_item_uri: Some("spotify:track:track-1".into()),
            item_type: "track".into(),
            added_at: None,
            unavailable_reason: None,
        }];
        database
            .replace_collection_entries(collection_id, &original)
            .expect("initial replacement should succeed");

        let invalid = vec![CollectionEntry {
            position: 0,
            source_track_id: Some(999_999),
            provider_item_uri: Some("spotify:track:missing".into()),
            item_type: "track".into(),
            added_at: None,
            unavailable_reason: None,
        }];
        assert!(
            database
                .replace_collection_entries(collection_id, &invalid)
                .is_err()
        );

        let connection = database.lock_connection().expect("database should lock");
        let persisted_track_id: i64 = connection
            .query_row(
                "SELECT source_track_id FROM collection_entries
                 WHERE collection_id = ?1 AND position = 0",
                params![collection_id],
                |row| row.get(0),
            )
            .expect("original entry should remain after rollback");

        assert_eq!(persisted_track_id, track_id);
    }
}
