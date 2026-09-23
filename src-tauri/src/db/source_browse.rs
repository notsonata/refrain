use rusqlite::{OptionalExtension, params};

use crate::domain::{
    SourceAccountOverview, SourceCollectionEntryView, SourceCollectionListPage,
    SourceCollectionPage, SourceCollectionSummary, SourceTrackView, SpotifySourceOverview,
};

use super::{Database, DatabaseError};

const MAX_PAGE_LIMIT: u32 = 500;

impl Database {
    pub fn spotify_source_overview(&self) -> Result<SpotifySourceOverview, DatabaseError> {
        self.with_connection(|connection| {
            let account = connection
                .query_row(
                    "SELECT id, display_name, last_source_sync_at
                     FROM source_accounts
                     WHERE provider = 'spotify'
                     ORDER BY updated_at DESC
                     LIMIT 1",
                    [],
                    |row| {
                        Ok((
                            row.get::<_, i64>(0)?,
                            SourceAccountOverview {
                                display_name: row.get(1)?,
                                last_source_sync_at: row.get(2)?,
                            },
                        ))
                    },
                )
                .optional()?;

            let Some((account_id, account)) = account else {
                return Ok(SpotifySourceOverview {
                    account: None,
                    liked_songs: None,
                    playlist_count: 0,
                });
            };

            let liked_songs = collection_summary_by_kind(connection, account_id, "liked_songs")?;
            let playlist_count = connection.query_row(
                "SELECT COUNT(*)
                 FROM source_collections
                 WHERE source_account_id = ?1 AND kind = 'playlist'",
                [account_id],
                |row| row.get(0),
            )?;

            Ok(SpotifySourceOverview {
                account: Some(account),
                liked_songs,
                playlist_count,
            })
        })
    }

    pub fn spotify_playlists_page(
        &self,
        offset: u32,
        limit: u32,
    ) -> Result<SourceCollectionListPage, DatabaseError> {
        let limit = normalize_limit(limit);
        self.with_connection(|connection| {
            let account_id = latest_spotify_account_id(connection)?;
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
                 WHERE source_account_id = ?1 AND kind = 'playlist'",
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
                   AND collection.kind = 'playlist'
                 GROUP BY collection.id
                 ORDER BY lower(collection.name), collection.id
                 LIMIT ?2 OFFSET ?3",
            )?;
            let rows = statement.query_map(
                params![account_id, i64::from(limit), i64::from(offset)],
                collection_summary_from_row,
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

    pub fn source_collection_page(
        &self,
        collection_id: i64,
        offset: u32,
        limit: u32,
    ) -> Result<Option<SourceCollectionPage>, DatabaseError> {
        let limit = normalize_limit(limit);
        self.with_connection(|connection| {
            let collection = connection
                .query_row(
                    "SELECT
                        collection.id,
                        collection.provider_collection_id,
                        collection.kind,
                        collection.name,
                        collection.is_accessible,
                        collection.access_issue,
                        COUNT(entries.id)
                     FROM source_collections AS collection
                     JOIN source_accounts AS account
                       ON account.id = collection.source_account_id
                     LEFT JOIN collection_entries AS entries
                       ON entries.collection_id = collection.id
                     WHERE collection.id = ?1
                       AND account.provider = 'spotify'
                     GROUP BY collection.id",
                    [collection_id],
                    collection_summary_from_row,
                )
                .optional()?;

            let Some(collection) = collection else {
                return Ok(None);
            };

            let mut statement = connection.prepare(
                "SELECT
                    entry.position,
                    entry.item_type,
                    entry.added_at,
                    entry.unavailable_reason,
                    track.provider_track_id,
                    track.title,
                    track.artists_json,
                    track.album,
                    track.duration_ms,
                    track.explicit,
                    track.image_url,
                    track.external_url
                 FROM collection_entries AS entry
                 LEFT JOIN source_tracks AS track
                   ON track.id = entry.source_track_id
                 WHERE entry.collection_id = ?1
                 ORDER BY entry.position
                 LIMIT ?2 OFFSET ?3",
            )?;
            let rows = statement.query_map(
                params![collection_id, i64::from(limit), i64::from(offset)],
                |row| {
                    let track = if let Some(provider_track_id) = row.get::<_, Option<String>>(4)? {
                        let artists_json = row
                            .get::<_, Option<String>>(6)?
                            .unwrap_or_else(|| "[]".to_owned());
                        let artists = serde_json::from_str::<Vec<String>>(&artists_json)
                            .unwrap_or_else(|_| Vec::new());

                        Some(SourceTrackView {
                            provider_track_id,
                            title: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
                            artists,
                            album: row.get(7)?,
                            duration_ms: row.get(8)?,
                            explicit: row.get::<_, Option<i64>>(9)?.map(|value| value != 0),
                            image_url: row.get(10)?,
                            external_url: row.get(11)?,
                        })
                    } else {
                        None
                    };

                    Ok(SourceCollectionEntryView {
                        position: row.get(0)?,
                        item_type: row.get(1)?,
                        added_at: row.get(2)?,
                        unavailable_reason: row.get(3)?,
                        track,
                    })
                },
            )?;
            let entries = rows.collect::<Result<Vec<_>, _>>()?;
            let total = collection.entry_count;

            Ok(Some(SourceCollectionPage {
                collection,
                entries,
                total,
                offset,
                limit,
            }))
        })
    }
}

fn latest_spotify_account_id(
    connection: &rusqlite::Connection,
) -> Result<Option<i64>, rusqlite::Error> {
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
}

fn collection_summary_by_kind(
    connection: &rusqlite::Connection,
    account_id: i64,
    kind: &str,
) -> Result<Option<SourceCollectionSummary>, rusqlite::Error> {
    connection
        .query_row(
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
               AND collection.kind = ?2
             GROUP BY collection.id
             ORDER BY collection.id
             LIMIT 1",
            params![account_id, kind],
            collection_summary_from_row,
        )
        .optional()
}

fn collection_summary_from_row(
    row: &rusqlite::Row<'_>,
) -> Result<SourceCollectionSummary, rusqlite::Error> {
    Ok(SourceCollectionSummary {
        id: row.get(0)?,
        provider_collection_id: row.get(1)?,
        kind: row.get(2)?,
        name: row.get(3)?,
        is_accessible: row.get::<_, i64>(4)? != 0,
        access_issue: row.get(5)?,
        entry_count: row.get(6)?,
    })
}

fn normalize_limit(limit: u32) -> u32 {
    limit.clamp(1, MAX_PAGE_LIMIT)
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
                    "refrain-source-browse-{name}-{}-{id}.sqlite3",
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

    fn sample_track(id: &str, title: &str) -> SourceTrack {
        SourceTrack {
            provider: "spotify".into(),
            provider_track_id: id.into(),
            uri: Some(format!("spotify:track:{id}")),
            isrc: None,
            title: title.into(),
            normalized_title: title.to_lowercase(),
            artists_json: "[\"Artist\",\"Guest\"]".into(),
            normalized_artists: "artist guest".into(),
            album: Some("Album".into()),
            normalized_album: Some("album".into()),
            duration_ms: Some(181_000),
            disc_number: Some(1),
            track_number: Some(1),
            release_year: Some(2026),
            explicit: Some(false),
            version_kind: None,
            version_detail: None,
            image_url: Some("https://example.test/cover.jpg".into()),
            external_url: Some(format!("https://open.spotify.com/track/{id}")),
        }
    }

    fn setup_database(name: &str) -> (TestDatabasePath, Database, i64) {
        let path = TestDatabasePath::new(name);
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
            .mark_source_account_synced(account_id, 1_790_000_000_000)
            .expect("sync timestamp should save");
        (path, database, account_id)
    }

    #[test]
    fn overview_and_playlist_pages_return_persisted_state() {
        let (_path, database, account_id) = setup_database("overview");
        database
            .replace_source_collection(
                &SourceCollection {
                    source_account_id: account_id,
                    provider_collection_id: "spotify:liked-songs".into(),
                    kind: "liked_songs".into(),
                    name: "Liked Songs".into(),
                    snapshot_id: None,
                    owner_provider_id: Some("listener".into()),
                    is_accessible: true,
                    access_issue: None,
                },
                &[SourceCollectionItem {
                    position: 0,
                    track: Some(sample_track("liked-track", "Liked Track")),
                    provider_item_uri: Some("spotify:track:liked-track".into()),
                    item_type: "track".into(),
                    added_at: None,
                    unavailable_reason: None,
                }],
            )
            .expect("liked songs should save");

        for (id, name, accessible) in [("playlist-b", "Beta", true), ("playlist-a", "Alpha", false)]
        {
            database
                .replace_source_collection(
                    &SourceCollection {
                        source_account_id: account_id,
                        provider_collection_id: id.into(),
                        kind: "playlist".into(),
                        name: name.into(),
                        snapshot_id: None,
                        owner_provider_id: Some("listener".into()),
                        is_accessible: accessible,
                        access_issue: (!accessible).then(|| "Spotify denied access".into()),
                    },
                    &[],
                )
                .expect("playlist should save");
        }

        let overview = database
            .spotify_source_overview()
            .expect("overview should load");
        assert_eq!(
            overview.account.unwrap().display_name.as_deref(),
            Some("Listener")
        );
        assert_eq!(overview.liked_songs.unwrap().entry_count, 1);
        assert_eq!(overview.playlist_count, 2);

        let playlists = database
            .spotify_playlists_page(0, 1)
            .expect("playlists should load");
        assert_eq!(playlists.total, 2);
        assert_eq!(playlists.items.len(), 1);
        assert_eq!(playlists.items[0].name, "Alpha");
        assert!(!playlists.items[0].is_accessible);
    }

    #[test]
    fn collection_page_preserves_order_duplicates_and_unavailable_positions() {
        let (_path, database, account_id) = setup_database("entries");
        let collection_id = database
            .replace_source_collection(
                &SourceCollection {
                    source_account_id: account_id,
                    provider_collection_id: "playlist".into(),
                    kind: "playlist".into(),
                    name: "Playlist".into(),
                    snapshot_id: Some("snapshot".into()),
                    owner_provider_id: Some("listener".into()),
                    is_accessible: true,
                    access_issue: None,
                },
                &[
                    SourceCollectionItem {
                        position: 0,
                        track: Some(sample_track("track", "Track")),
                        provider_item_uri: Some("spotify:track:track".into()),
                        item_type: "track".into(),
                        added_at: None,
                        unavailable_reason: None,
                    },
                    SourceCollectionItem {
                        position: 1,
                        track: Some(sample_track("track", "Track")),
                        provider_item_uri: Some("spotify:track:track".into()),
                        item_type: "track".into(),
                        added_at: None,
                        unavailable_reason: None,
                    },
                    SourceCollectionItem {
                        position: 2,
                        track: None,
                        provider_item_uri: None,
                        item_type: "unavailable".into(),
                        added_at: None,
                        unavailable_reason: Some("removed".into()),
                    },
                ],
            )
            .expect("playlist should save");

        let page = database
            .source_collection_page(collection_id, 0, 50)
            .expect("collection should load")
            .expect("collection should exist");

        assert_eq!(page.total, 3);
        assert_eq!(page.entries.len(), 3);
        assert_eq!(page.entries[0].position, 0);
        assert_eq!(page.entries[1].position, 1);
        assert_eq!(
            page.entries[0].track.as_ref().unwrap().provider_track_id,
            page.entries[1].track.as_ref().unwrap().provider_track_id
        );
        assert_eq!(
            page.entries[0].track.as_ref().unwrap().artists,
            vec!["Artist", "Guest"]
        );
        assert!(page.entries[2].track.is_none());
        assert_eq!(
            page.entries[2].unavailable_reason.as_deref(),
            Some("removed")
        );
    }
}
