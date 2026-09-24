use std::collections::HashSet;

use rusqlite::{OptionalExtension, params};

use crate::domain::{
    SourceCollection, SourceCollectionItem, SourceCollectionListPage, SourceCollectionSummary,
};

use super::{Database, DatabaseError, source};

const MAX_PAGE_LIMIT: u32 = 500;

impl Database {
    pub fn latest_spotify_source_account_id(&self) -> Result<Option<i64>, DatabaseError> {
        self.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT id
                     FROM source_accounts
                     WHERE provider = 'spotify'
                     ORDER BY updated_at DESC
                     LIMIT 1",
                    [],
                    |row| row.get(0),
                )
                .optional()
        })
    }

    pub fn replace_spotify_saved_albums(
        &self,
        source_account_id: i64,
        albums: &[(SourceCollection, Vec<SourceCollectionItem>)],
    ) -> Result<usize, DatabaseError> {
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        let provider_collection_ids = albums
            .iter()
            .map(|(collection, _)| collection.provider_collection_id.clone())
            .collect::<HashSet<_>>();

        for (collection, items) in albums {
            let collection_id = source::upsert_collection(&transaction, collection)?;
            transaction.execute(
                "DELETE FROM collection_entries WHERE collection_id = ?1",
                [collection_id],
            )?;

            for item in items {
                let source_track_id = if let Some(track) = item.track.as_ref() {
                    let existing_isrc = transaction
                        .query_row(
                            "SELECT isrc
                             FROM source_tracks
                             WHERE provider = ?1 AND provider_track_id = ?2",
                            params![track.provider, track.provider_track_id],
                            |row| row.get::<_, Option<String>>(0),
                        )
                        .optional()?
                        .flatten();
                    let mut merged_track = track.clone();
                    if merged_track.isrc.is_none() {
                        merged_track.isrc = existing_isrc;
                    }
                    Some(source::upsert_track(&transaction, &merged_track)?)
                } else {
                    None
                };

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
        }

        let existing = {
            let mut statement = transaction.prepare(
                "SELECT id, provider_collection_id
                 FROM source_collections
                 WHERE source_account_id = ?1 AND kind = 'saved_album'",
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

    pub fn spotify_saved_albums_page(
        &self,
        offset: u32,
        limit: u32,
    ) -> Result<SourceCollectionListPage, DatabaseError> {
        let limit = limit.clamp(1, MAX_PAGE_LIMIT);
        self.with_connection(|connection| {
            let account_id = connection
                .query_row(
                    "SELECT id
                     FROM source_accounts
                     WHERE provider = 'spotify'
                     ORDER BY updated_at DESC
                     LIMIT 1",
                    [],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?;
            let Some(account_id) = account_id else {
                return Ok(SourceCollectionListPage {
                    items: Vec::new(),
                    total: 0,
                    offset,
                    limit,
                });
            };

            let total = connection.query_row(
                "SELECT COUNT(*)
                 FROM source_collections
                 WHERE source_account_id = ?1 AND kind = 'saved_album'",
                [account_id],
                |row| row.get(0),
            )?;

            let mut statement = connection.prepare(
                "SELECT
                    collection.id,
                    collection.provider_collection_id,
                    collection.kind,
                    collection.name,
                    collection.is_accessible,
                    collection.access_issue,
                    COUNT(entries.id)
                 FROM source_collections AS collection
                 LEFT JOIN collection_entries AS entries
                   ON entries.collection_id = collection.id
                 WHERE collection.source_account_id = ?1
                   AND collection.kind = 'saved_album'
                 GROUP BY collection.id
                 ORDER BY lower(collection.name), collection.id
                 LIMIT ?2 OFFSET ?3",
            )?;
            let rows = statement.query_map(
                params![account_id, i64::from(limit), i64::from(offset)],
                |row| {
                    Ok(SourceCollectionSummary {
                        id: row.get(0)?,
                        provider_collection_id: row.get(1)?,
                        kind: row.get(2)?,
                        name: row.get(3)?,
                        is_accessible: row.get::<_, i64>(4)? != 0,
                        access_issue: row.get(5)?,
                        entry_count: row.get(6)?,
                    })
                },
            )?;
            let items = rows.collect::<Result<Vec<_>, _>>()?;

            Ok(SourceCollectionListPage {
                items,
                total,
                offset,
                limit,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use crate::domain::{SourceAccount, SourceTrack};

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
                    "refrain-saved-albums-{name}-{}-{id}.sqlite3",
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

    fn album(account_id: i64, id: &str, name: &str) -> (SourceCollection, Vec<SourceCollectionItem>) {
        (
            SourceCollection {
                source_account_id: account_id,
                provider_collection_id: format!("spotify:album:{id}"),
                kind: "saved_album".into(),
                name: name.into(),
                snapshot_id: None,
                owner_provider_id: None,
                is_accessible: true,
                access_issue: None,
            },
            vec![SourceCollectionItem {
                position: 0,
                track: Some(sample_track(id)),
                provider_item_uri: Some(format!("spotify:track:{id}")),
                item_type: "track".into(),
                added_at: None,
                unavailable_reason: None,
            }],
        )
    }

    #[test]
    fn saved_album_replace_is_atomic_and_removes_stale_albums() {
        let path = TestDatabasePath::new("replace");
        let database = Database::open(path.path.clone()).expect("database should open");
        let account_id = database
            .upsert_source_account(&SourceAccount {
                provider: "spotify".into(),
                provider_account_id: "listener".into(),
                display_name: Some("Listener".into()),
                client_id: "client".into(),
            })
            .expect("account should save");

        database
            .replace_spotify_saved_albums(
                account_id,
                &[album(account_id, "one", "One"), album(account_id, "two", "Two")],
            )
            .expect("albums should save");
        let removed = database
            .replace_spotify_saved_albums(account_id, &[album(account_id, "two", "Two")])
            .expect("albums should replace");

        assert_eq!(removed, 1);
        let page = database
            .spotify_saved_albums_page(0, 50)
            .expect("albums should load");
        assert_eq!(page.total, 1);
        assert_eq!(page.items[0].name, "Two");
        assert_eq!(page.items[0].entry_count, 1);
    }
}
