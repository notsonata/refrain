// Milestone 12 persistence is exercised by the acquisition boundary before production wiring.
#[allow(dead_code)]
mod acquisition;
mod issues;
mod local_library;
mod local_playlists;
mod matching;
mod migrations;
mod normalization;
mod playlist_exports;
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

pub(crate) use acquisition::AcquisitionImportMetadata;
pub(crate) use local_library::LocalFileWrite;
#[cfg(test)]
use migrations::MIGRATION_COUNT;
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

        let database = Self {
            path,
            connection: Mutex::new(connection),
        };
        database.sync_tracked_spotify_playlist_mirrors()?;
        Ok(database)
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
        assert_eq!(user_version, MIGRATION_COUNT);

        for table in [
            "library_tracks",
            "local_files",
            "local_playlists",
            "local_playlist_entries",
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
                acquisition_providers: vec!["sockseek".into(), "monochrome".into()],
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
            settings.acquisition_providers,
            vec!["sockseek", "monochrome"]
        );
        assert_eq!(
            settings.spotify_client_id.as_deref(),
            Some("spotify-client")
        );
    }

    #[test]
    fn local_playlists_preserve_entry_order_and_duplicates() {
        let test_path = TestDatabasePath::new("local-playlists");
        let database = Database::open(test_path.path.clone()).expect("database should open");
        let connection = database.lock_connection().expect("database should lock");
        let now = now_ms();

        let mut track_ids = Vec::new();
        for (index, title) in ["First", "Second"].into_iter().enumerate() {
            connection
                .execute(
                    "INSERT INTO library_tracks (
                        title, normalized_title, artists_json, normalized_artists,
                        duration_ms, created_at, updated_at
                     ) VALUES (?1, ?2, '[\"Artist\"]', 'artist', 180000, ?3, ?3)",
                    params![title, title.to_ascii_lowercase(), now],
                )
                .expect("library track should insert");
            let track_id = connection.last_insert_rowid();
            connection
                .execute(
                    "INSERT INTO local_files (
                        library_track_id, path, ownership, is_preferred, state,
                        file_size, modified_at, created_at, updated_at
                     ) VALUES (?1, ?2, 'external', 1, 'present', 100, 1, ?3, ?3)",
                    params![track_id, format!("/music/{index}.flac"), now],
                )
                .expect("local file should insert");
            track_ids.push(track_id);
        }
        drop(connection);

        let playlist = database
            .create_local_playlist("Road Trip")
            .expect("playlist should be created");
        let updated = database
            .add_tracks_to_local_playlist(playlist.id, &[track_ids[0], track_ids[1], track_ids[0]])
            .expect("tracks should be added");

        assert_eq!(updated.name, "Road Trip");
        assert_eq!(updated.entries.len(), 3);
        assert_eq!(
            updated
                .entries
                .iter()
                .map(|entry| entry.library_track_id)
                .collect::<Vec<_>>(),
            vec![track_ids[0], track_ids[1], track_ids[0]]
        );
        assert_eq!(
            updated
                .entries
                .iter()
                .map(|entry| entry.position)
                .collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
    }

    #[test]
    fn local_playlist_entries_can_be_moved_and_removed() {
        let test_path = TestDatabasePath::new("local-playlist-editing");
        let database = Database::open(test_path.path.clone()).expect("database should open");
        let connection = database.lock_connection().expect("database should lock");
        let now = now_ms();
        let mut track_ids = Vec::new();

        for title in ["One", "Two", "Three"] {
            connection
                .execute(
                    "INSERT INTO library_tracks (
                        title, normalized_title, artists_json, normalized_artists,
                        created_at, updated_at
                     ) VALUES (?1, ?2, '[\"Artist\"]', 'artist', ?3, ?3)",
                    params![title, title.to_ascii_lowercase(), now],
                )
                .expect("library track should insert");
            let track_id = connection.last_insert_rowid();
            connection
                .execute(
                    "INSERT INTO local_files (
                        library_track_id, path, ownership, is_preferred, state,
                        file_size, modified_at, created_at, updated_at
                     ) VALUES (?1, ?2, 'external', 1, 'present', 100, 1, ?3, ?3)",
                    params![track_id, format!("/music/{title}.flac"), now],
                )
                .expect("local file should insert");
            track_ids.push(track_id);
        }
        drop(connection);

        let playlist = database
            .create_local_playlist("Order")
            .expect("playlist should be created");
        let playlist = database
            .add_tracks_to_local_playlist(playlist.id, &track_ids)
            .expect("tracks should be added");
        let third_entry_id = playlist.entries[2].id;
        let second_entry_id = playlist.entries[1].id;

        let reordered = database
            .move_local_playlist_entry(playlist.id, third_entry_id, 0)
            .expect("entry should move");
        assert_eq!(
            reordered
                .entries
                .iter()
                .map(|entry| entry.library_track_id)
                .collect::<Vec<_>>(),
            vec![track_ids[2], track_ids[0], track_ids[1]]
        );

        let removed = database
            .remove_local_playlist_entry(playlist.id, second_entry_id)
            .expect("entry should be removed");
        assert_eq!(
            removed
                .entries
                .iter()
                .map(|entry| entry.library_track_id)
                .collect::<Vec<_>>(),
            vec![track_ids[2], track_ids[0]]
        );
        assert_eq!(
            removed
                .entries
                .iter()
                .map(|entry| entry.position)
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
    }

    #[test]
    fn local_playlist_metadata_can_be_updated_listed_and_deleted() {
        let test_path = TestDatabasePath::new("local-playlist-metadata");
        let database = Database::open(test_path.path.clone()).expect("database should open");

        let playlist = database
            .create_local_playlist("Draft")
            .expect("playlist should be created");
        let renamed = database
            .rename_local_playlist(playlist.id, "Final")
            .expect("playlist should be renamed");
        let targeted = database
            .set_local_playlist_m3u_path(playlist.id, Some("/music/playlists/final.m3u8"))
            .expect("playlist target should update");

        assert_eq!(renamed.name, "Final");
        assert_eq!(
            targeted.m3u_path.as_deref(),
            Some("/music/playlists/final.m3u8")
        );
        let summaries = database
            .list_local_playlists()
            .expect("playlists should list");
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].name, "Final");
        assert_eq!(summaries[0].entry_count, 0);

        database
            .delete_local_playlist(playlist.id)
            .expect("playlist should delete");
        assert!(
            database
                .list_local_playlists()
                .expect("playlists should list")
                .is_empty()
        );
    }

    #[test]
    fn tracked_spotify_playlist_creates_and_updates_a_local_mirror() {
        let test_path = TestDatabasePath::new("spotify-playlist-mirror");
        let database = Database::open(test_path.path.clone()).expect("database should open");
        let account_id = database
            .upsert_source_account(&sample_account())
            .expect("source account should persist");
        let mut source_playlist = sample_collection(account_id);
        source_playlist.image_url = Some("https://i.scdn.co/image/playlist-cover".into());
        let collection_id = database
            .upsert_source_collection(&source_playlist)
            .expect("source collection should persist");
        let first = database
            .upsert_source_track(&sample_track("track-1"))
            .expect("first source track should persist");
        let second = database
            .upsert_source_track(&sample_track("track-2"))
            .expect("second source track should persist");
        database
            .replace_collection_entries(
                collection_id,
                &[
                    CollectionEntry {
                        position: 0,
                        source_track_id: Some(first),
                        provider_item_uri: None,
                        item_type: "track".into(),
                        added_at: None,
                        unavailable_reason: None,
                    },
                    CollectionEntry {
                        position: 1,
                        source_track_id: Some(first),
                        provider_item_uri: None,
                        item_type: "track".into(),
                        added_at: None,
                        unavailable_reason: None,
                    },
                    CollectionEntry {
                        position: 2,
                        source_track_id: Some(second),
                        provider_item_uri: None,
                        item_type: "track".into(),
                        added_at: None,
                        unavailable_reason: None,
                    },
                ],
            )
            .expect("source entries should persist");
        let first_library = database
            .create_library_track_from_source(first)
            .expect("first library identity should persist");
        let second_library = database
            .create_library_track_from_source(second)
            .expect("second library identity should persist");
        database
            .persist_track_link(first, first_library, "existing", 10_000)
            .expect("first link should persist");
        database
            .persist_track_link(second, second_library, "existing", 10_000)
            .expect("second link should persist");

        let manual = database
            .create_local_playlist("Manual")
            .expect("manual playlist should persist");
        database
            .set_source_collection_tracking(collection_id, true)
            .expect("playlist tracking should persist");
        database
            .sync_tracked_spotify_playlist_mirrors()
            .expect("tracked playlist mirror should synchronize");

        let summaries = database
            .list_local_playlists()
            .expect("local playlists should list");
        let mirrored = summaries
            .iter()
            .find(|playlist| playlist.source_collection_id == Some(collection_id))
            .expect("tracked Spotify playlist should create a local mirror");
        assert_eq!(
            mirrored.image_url.as_deref(),
            Some("https://i.scdn.co/image/playlist-cover")
        );
        let detail = database
            .get_local_playlist(mirrored.id)
            .expect("mirrored playlist should load");
        assert_eq!(detail.name, "Playlist");
        assert_eq!(
            detail.image_url.as_deref(),
            Some("https://i.scdn.co/image/playlist-cover")
        );
        assert_eq!(
            detail
                .entries
                .iter()
                .map(|entry| entry.library_track_id)
                .collect::<Vec<_>>(),
            vec![first_library, first_library, second_library]
        );

        database
            .replace_collection_entries(
                collection_id,
                &[
                    CollectionEntry {
                        position: 0,
                        source_track_id: Some(second),
                        provider_item_uri: None,
                        item_type: "track".into(),
                        added_at: None,
                        unavailable_reason: None,
                    },
                    CollectionEntry {
                        position: 1,
                        source_track_id: Some(first),
                        provider_item_uri: None,
                        item_type: "track".into(),
                        added_at: None,
                        unavailable_reason: None,
                    },
                ],
            )
            .expect("updated source entries should persist");
        database
            .sync_tracked_spotify_playlist_mirrors()
            .expect("changed Spotify membership should update the mirror");
        let updated = database
            .get_local_playlist(mirrored.id)
            .expect("updated mirror should load");
        assert_eq!(
            updated
                .entries
                .iter()
                .map(|entry| entry.library_track_id)
                .collect::<Vec<_>>(),
            vec![second_library, first_library]
        );

        database
            .set_source_track_tracking(collection_id, first, Some(false))
            .expect("track exclusion should persist");
        database
            .sync_tracked_spotify_playlist_mirrors()
            .expect("track exclusion should update the mirror");
        let excluded = database
            .get_local_playlist(mirrored.id)
            .expect("excluded mirror should load");
        assert_eq!(
            excluded
                .entries
                .iter()
                .map(|entry| entry.library_track_id)
                .collect::<Vec<_>>(),
            vec![second_library]
        );

        database
            .set_source_collection_tracking(collection_id, false)
            .expect("playlist untracking should persist");
        database
            .sync_tracked_spotify_playlist_mirrors()
            .expect("untracking should remove the derived mirror");
        let remaining = database
            .list_local_playlists()
            .expect("remaining playlists should list");
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, manual.id);
        assert_eq!(remaining[0].source_collection_id, None);
    }

    #[test]
    fn tracked_liked_songs_creates_and_removes_a_local_mirror() {
        let test_path = TestDatabasePath::new("liked-songs-mirror");
        let database = Database::open(test_path.path.clone()).expect("database should open");
        let account_id = database
            .upsert_source_account(&sample_account())
            .expect("source account should persist");
        let mut liked_songs = sample_collection(account_id);
        liked_songs.provider_collection_id = "spotify:liked-songs".into();
        liked_songs.kind = "liked_songs".into();
        liked_songs.name = "Liked Songs".into();
        liked_songs.image_url = None;
        let collection_id = database
            .upsert_source_collection(&liked_songs)
            .expect("Liked Songs should persist");
        let first = database
            .upsert_source_track(&sample_track("liked-1"))
            .expect("first source track should persist");
        let second = database
            .upsert_source_track(&sample_track("liked-2"))
            .expect("second source track should persist");
        database
            .replace_collection_entries(
                collection_id,
                &[
                    CollectionEntry {
                        position: 0,
                        source_track_id: Some(first),
                        provider_item_uri: None,
                        item_type: "track".into(),
                        added_at: None,
                        unavailable_reason: None,
                    },
                    CollectionEntry {
                        position: 1,
                        source_track_id: Some(second),
                        provider_item_uri: None,
                        item_type: "track".into(),
                        added_at: None,
                        unavailable_reason: None,
                    },
                ],
            )
            .expect("Liked Songs entries should persist");
        let first_library = database
            .create_library_track_from_source(first)
            .expect("first library identity should persist");
        let second_library = database
            .create_library_track_from_source(second)
            .expect("second library identity should persist");
        database
            .persist_track_link(first, first_library, "existing", 10_000)
            .expect("first link should persist");
        database
            .persist_track_link(second, second_library, "existing", 10_000)
            .expect("second link should persist");

        database
            .set_source_collection_tracking(collection_id, true)
            .expect("Liked Songs tracking should persist");
        database
            .sync_tracked_spotify_playlist_mirrors()
            .expect("tracked Liked Songs mirror should synchronize");

        let mirrored = database
            .list_local_playlists()
            .expect("local playlists should list")
            .into_iter()
            .find(|playlist| playlist.source_collection_id == Some(collection_id))
            .expect("tracked Liked Songs should create a local mirror");
        assert_eq!(mirrored.name, "Liked Songs");
        let detail = database
            .get_local_playlist(mirrored.id)
            .expect("Liked Songs mirror should load");
        assert_eq!(
            detail
                .entries
                .iter()
                .map(|entry| entry.library_track_id)
                .collect::<Vec<_>>(),
            vec![first_library, second_library]
        );

        database
            .set_source_track_tracking(collection_id, first, Some(false))
            .expect("Liked Songs exclusion should persist");
        database
            .sync_tracked_spotify_playlist_mirrors()
            .expect("Liked Songs exclusion should update the mirror");
        let excluded = database
            .get_local_playlist(mirrored.id)
            .expect("updated Liked Songs mirror should load");
        assert_eq!(
            excluded
                .entries
                .iter()
                .map(|entry| entry.library_track_id)
                .collect::<Vec<_>>(),
            vec![second_library]
        );

        database
            .set_source_collection_tracking(collection_id, false)
            .expect("Liked Songs untracking should persist");
        database
            .sync_tracked_spotify_playlist_mirrors()
            .expect("untracking Liked Songs should remove the mirror");
        assert!(
            database
                .list_local_playlists()
                .expect("local playlists should list")
                .into_iter()
                .all(|playlist| playlist.source_collection_id != Some(collection_id))
        );
    }

    #[test]
    fn spotify_playlist_mirrors_reject_direct_content_edits() {
        let test_path = TestDatabasePath::new("spotify-playlist-mirror-readonly");
        let database = Database::open(test_path.path.clone()).expect("database should open");
        let account_id = database
            .upsert_source_account(&sample_account())
            .expect("source account should persist");
        let collection_id = database
            .upsert_source_collection(&sample_collection(account_id))
            .expect("source collection should persist");
        database
            .set_source_collection_tracking(collection_id, true)
            .expect("playlist tracking should persist");
        database
            .sync_tracked_spotify_playlist_mirrors()
            .expect("tracked playlist mirror should synchronize");
        let mirrored = database
            .list_local_playlists()
            .expect("local playlists should list")
            .into_iter()
            .find(|playlist| playlist.source_collection_id == Some(collection_id))
            .expect("mirror should exist");

        let rename_error = database
            .rename_local_playlist(mirrored.id, "Edited")
            .expect_err("Spotify mirror rename should be rejected");
        assert!(
            rename_error
                .to_string()
                .contains("cannot be edited directly")
        );
        let delete_error = database
            .delete_local_playlist(mirrored.id)
            .expect_err("Spotify mirror deletion should be rejected");
        assert!(
            delete_error
                .to_string()
                .contains("cannot be edited directly")
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
