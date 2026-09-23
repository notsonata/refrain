use std::collections::HashSet;

use rusqlite::{OptionalExtension, params};

use crate::domain::{SourceCollection, SourceCollectionItem};

use super::{Database, DatabaseError, now_ms, source};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSourceCollectionState {
    pub snapshot_id: Option<String>,
    pub is_accessible: bool,
    pub entry_count: usize,
}

impl Database {
    pub fn source_collection_state(
        &self,
        source_account_id: i64,
        provider_collection_id: &str,
    ) -> Result<Option<StoredSourceCollectionState>, DatabaseError> {
        self.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT
                        collection.snapshot_id,
                        collection.is_accessible,
                        COUNT(entries.id)
                     FROM source_collections AS collection
                     LEFT JOIN collection_entries AS entries
                       ON entries.collection_id = collection.id
                     WHERE collection.source_account_id = ?1
                       AND collection.provider_collection_id = ?2
                     GROUP BY collection.id",
                    params![source_account_id, provider_collection_id],
                    |row| {
                        Ok(StoredSourceCollectionState {
                            snapshot_id: row.get(0)?,
                            is_accessible: row.get::<_, i64>(1)? != 0,
                            entry_count: row.get::<_, i64>(2)? as usize,
                        })
                    },
                )
                .optional()
        })
    }

    pub fn replace_source_collection(
        &self,
        collection: &SourceCollection,
        items: &[SourceCollectionItem],
    ) -> Result<i64, DatabaseError> {
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        let collection_id = source::upsert_collection(&transaction, collection)?;

        transaction.execute(
            "DELETE FROM collection_entries WHERE collection_id = ?1",
            [collection_id],
        )?;

        for item in items {
            let source_track_id = item
                .track
                .as_ref()
                .map(|track| source::upsert_track(&transaction, track))
                .transpose()?;

            transaction.execute(
                "INSERT INTO collection_entries (
                    collection_id,
                    position,
                    source_track_id,
                    provider_item_uri,
                    item_type,
                    added_at,
                    unavailable_reason
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    collection_id,
                    item.position,
                    source_track_id,
                    item.provider_item_uri,
                    item.item_type,
                    item.added_at,
                    item.unavailable_reason,
                ],
            )?;
        }

        transaction.commit()?;
        Ok(collection_id)
    }

    pub fn retain_source_playlists(
        &self,
        source_account_id: i64,
        provider_collection_ids: &HashSet<String>,
    ) -> Result<usize, DatabaseError> {
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        let existing = {
            let mut statement = transaction.prepare(
                "SELECT id, provider_collection_id
                 FROM source_collections
                 WHERE source_account_id = ?1 AND kind = 'playlist'",
            )?;
            let rows = statement.query_map([source_account_id], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
            })?;
            rows.collect::<Result<Vec<_>, _>>()?
        };

        let mut removed = 0;
        for (collection_id, provider_collection_id) in existing {
            if !provider_collection_ids.contains(&provider_collection_id) {
                transaction.execute(
                    "DELETE FROM source_collections WHERE id = ?1",
                    [collection_id],
                )?;
                removed += 1;
            }
        }

        transaction.commit()?;
        Ok(removed)
    }

    pub fn mark_source_account_synced_now(&self, account_id: i64) -> Result<i64, DatabaseError> {
        let synced_at = now_ms();
        self.mark_source_account_synced(account_id, synced_at)?;
        Ok(synced_at)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use crate::domain::{SourceAccount, SourceCollection, SourceCollectionItem, SourceTrack};

    use super::*;

    static NEXT_DATABASE_ID: AtomicU64 = AtomicU64::new(0);

    struct TestDatabasePath {
        path: PathBuf,
    }

    impl TestDatabasePath {
        fn new(name: &str) -> Self {
            let id = NEXT_DATABASE_ID.fetch_add(1, Ordering::Relaxed);
            Self {
                path: std::env::temp_dir().join(format!(
                    "refrain-source-refresh-{name}-{}-{id}.sqlite3",
                    std::process::id()
                )),
            }
        }
    }

    impl Drop for TestDatabasePath {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.path);
            let _ = fs::remove_file(self.path.with_extension("sqlite3-wal"));
            let _ = fs::remove_file(self.path.with_extension("sqlite3-shm"));
        }
    }

    fn sample_track(id: &str) -> SourceTrack {
        SourceTrack {
            provider: "spotify".into(),
            provider_track_id: id.into(),
            uri: Some(format!("spotify:track:{id}")),
            isrc: None,
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

    fn setup_database(name: &str) -> (TestDatabasePath, Database, i64) {
        let path = TestDatabasePath::new(name);
        let database = Database::open(path.path.clone()).expect("database should open");
        let account_id = database
            .upsert_source_account(&SourceAccount {
                provider: "spotify".into(),
                provider_account_id: "account".into(),
                display_name: Some("Listener".into()),
                client_id: "client".into(),
            })
            .expect("account should save");
        (path, database, account_id)
    }

    #[test]
    fn collection_metadata_tracks_and_entries_replace_atomically() {
        let (_path, database, account_id) = setup_database("atomic");
        let collection = SourceCollection {
            source_account_id: account_id,
            provider_collection_id: "playlist".into(),
            kind: "playlist".into(),
            name: "Playlist".into(),
            snapshot_id: Some("snapshot-one".into()),
            owner_provider_id: Some("account".into()),
            is_accessible: true,
            access_issue: None,
        };
        let first_items = vec![SourceCollectionItem {
            position: 0,
            track: Some(sample_track("track-one")),
            provider_item_uri: Some("spotify:track:track-one".into()),
            item_type: "track".into(),
            added_at: None,
            unavailable_reason: None,
        }];

        database
            .replace_source_collection(&collection, &first_items)
            .expect("first replacement should succeed");

        let mut updated = collection.clone();
        updated.snapshot_id = Some("snapshot-two".into());
        let invalid_items = vec![
            SourceCollectionItem {
                position: 0,
                track: Some(sample_track("track-two")),
                provider_item_uri: Some("spotify:track:track-two".into()),
                item_type: "track".into(),
                added_at: None,
                unavailable_reason: None,
            },
            SourceCollectionItem {
                position: 0,
                track: Some(sample_track("track-three")),
                provider_item_uri: Some("spotify:track:track-three".into()),
                item_type: "track".into(),
                added_at: None,
                unavailable_reason: None,
            },
        ];

        let error = database.replace_source_collection(&updated, &invalid_items);
        assert!(error.is_err());

        let state = database
            .source_collection_state(account_id, "playlist")
            .expect("state should load")
            .expect("collection should exist");
        assert_eq!(state.snapshot_id.as_deref(), Some("snapshot-one"));
        assert_eq!(state.entry_count, 1);
    }

    #[test]
    fn retain_playlists_preserves_liked_songs_and_current_playlists() {
        let (_path, database, account_id) = setup_database("retain");
        for (provider_id, kind) in [
            ("spotify:liked-songs", "liked_songs"),
            ("keep", "playlist"),
            ("remove", "playlist"),
        ] {
            database
                .upsert_source_collection(&SourceCollection {
                    source_account_id: account_id,
                    provider_collection_id: provider_id.into(),
                    kind: kind.into(),
                    name: provider_id.into(),
                    snapshot_id: None,
                    owner_provider_id: Some("account".into()),
                    is_accessible: true,
                    access_issue: None,
                })
                .expect("collection should save");
        }

        let removed = database
            .retain_source_playlists(account_id, &HashSet::from(["keep".to_string()]))
            .expect("retention should succeed");

        assert_eq!(removed, 1);
        assert!(
            database
                .source_collection_state(account_id, "spotify:liked-songs")
                .expect("liked state should load")
                .is_some()
        );
        assert!(
            database
                .source_collection_state(account_id, "keep")
                .expect("playlist state should load")
                .is_some()
        );
        assert!(
            database
                .source_collection_state(account_id, "remove")
                .expect("removed state should load")
                .is_none()
        );
    }
}
