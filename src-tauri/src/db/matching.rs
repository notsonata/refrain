use std::collections::HashSet;

use rusqlite::{OptionalExtension, Row, params};

use crate::{domain::MatchTrackDescriptor, matching::PersistedTrackLink};

use super::{Database, DatabaseError, now_ms};

impl Database {
    pub(crate) fn source_match_track(
        &self,
        source_track_id: i64,
    ) -> Result<Option<MatchTrackDescriptor>, DatabaseError> {
        self.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT
                        id, title, artists_json, album, isrc, duration_ms, disc_number,
                        track_number, explicit, version_kind, version_detail
                     FROM source_tracks
                     WHERE id = ?1",
                    [source_track_id],
                    match_track_from_row,
                )
                .optional()
        })
    }

    pub(crate) fn library_match_tracks(&self) -> Result<Vec<MatchTrackDescriptor>, DatabaseError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT
                    id, title, artists_json, album, isrc, duration_ms, disc_number,
                    track_number, explicit, version_kind, version_detail
                 FROM library_tracks
                 ORDER BY id",
            )?;
            statement
                .query_map([], match_track_from_row)?
                .collect::<Result<Vec<_>, _>>()
        })
    }

    pub(crate) fn persisted_track_link(
        &self,
        source_track_id: i64,
    ) -> Result<Option<PersistedTrackLink>, DatabaseError> {
        self.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT library_track_id, method, confidence, confirmed_by_user
                     FROM track_links
                     WHERE source_track_id = ?1",
                    [source_track_id],
                    |row| {
                        Ok(PersistedTrackLink {
                            library_track_id: row.get(0)?,
                            method: row.get(1)?,
                            confidence: row.get(2)?,
                            confirmed_by_user: row.get::<_, i64>(3)? != 0,
                        })
                    },
                )
                .optional()
        })
    }

    pub(crate) fn rejected_library_track_ids(
        &self,
        source_track_id: i64,
    ) -> Result<HashSet<i64>, DatabaseError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT library_track_id
                 FROM track_rejections
                 WHERE source_track_id = ?1
                 ORDER BY library_track_id",
            )?;
            let rows = statement.query_map([source_track_id], |row| row.get::<_, i64>(0))?;
            rows.collect::<Result<HashSet<_>, _>>()
        })
    }

    pub fn confirm_match(
        &self,
        source_track_id: i64,
        library_track_id: i64,
    ) -> Result<(), DatabaseError> {
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        let now = now_ms();
        transaction.execute(
            "DELETE FROM track_rejections
             WHERE source_track_id = ?1 AND library_track_id = ?2",
            params![source_track_id, library_track_id],
        )?;
        transaction.execute(
            "INSERT INTO track_links (
                source_track_id, library_track_id, method, confidence, confirmed_by_user,
                created_at, updated_at
             ) VALUES (?1, ?2, 'user', 10000, 1, ?3, ?3)
             ON CONFLICT(source_track_id) DO UPDATE SET
                library_track_id = excluded.library_track_id,
                method = 'user',
                confidence = 10000,
                confirmed_by_user = 1,
                updated_at = excluded.updated_at",
            params![source_track_id, library_track_id, now],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn reject_match(
        &self,
        source_track_id: i64,
        library_track_id: i64,
        reason: Option<&str>,
    ) -> Result<(), DatabaseError> {
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "DELETE FROM track_links
             WHERE source_track_id = ?1 AND library_track_id = ?2",
            params![source_track_id, library_track_id],
        )?;
        transaction.execute(
            "INSERT INTO track_rejections (
                source_track_id, library_track_id, reason, created_at
             ) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(source_track_id, library_track_id) DO UPDATE SET
                reason = excluded.reason",
            params![source_track_id, library_track_id, reason, now_ms()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn clear_match_decision(
        &self,
        source_track_id: i64,
        library_track_id: Option<i64>,
    ) -> Result<(), DatabaseError> {
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        match library_track_id {
            Some(library_track_id) => {
                transaction.execute(
                    "DELETE FROM track_links
                     WHERE source_track_id = ?1
                       AND library_track_id = ?2
                       AND confirmed_by_user = 1",
                    params![source_track_id, library_track_id],
                )?;
                transaction.execute(
                    "DELETE FROM track_rejections
                     WHERE source_track_id = ?1 AND library_track_id = ?2",
                    params![source_track_id, library_track_id],
                )?;
            }
            None => {
                transaction.execute(
                    "DELETE FROM track_links
                     WHERE source_track_id = ?1 AND confirmed_by_user = 1",
                    [source_track_id],
                )?;
                transaction.execute(
                    "DELETE FROM track_rejections WHERE source_track_id = ?1",
                    [source_track_id],
                )?;
            }
        }
        transaction.commit()?;
        Ok(())
    }
}

fn match_track_from_row(row: &Row<'_>) -> Result<MatchTrackDescriptor, rusqlite::Error> {
    let artists_json = row.get::<_, String>(2)?;
    Ok(MatchTrackDescriptor {
        id: row.get(0)?,
        title: row.get(1)?,
        artists: serde_json::from_str(&artists_json).unwrap_or_default(),
        album: row.get(3)?,
        isrc: row.get(4)?,
        duration_ms: row.get(5)?,
        disc_number: row.get(6)?,
        track_number: row.get(7)?,
        explicit: row.get::<_, Option<i64>>(8)?.map(|value| value != 0),
        version_kind: row.get(9)?,
        version_detail: row.get(10)?,
    })
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use rusqlite::params;

    use super::*;

    static NEXT_DATABASE_ID: AtomicU64 = AtomicU64::new(0);

    struct TestDatabasePath(PathBuf);

    impl TestDatabasePath {
        fn new() -> Self {
            let id = NEXT_DATABASE_ID.fetch_add(1, Ordering::Relaxed);
            Self(std::env::temp_dir().join(format!(
                "refrain-matching-db-{}-{id}.sqlite3",
                std::process::id()
            )))
        }
    }

    impl Drop for TestDatabasePath {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
            let _ = fs::remove_file(self.0.with_extension("sqlite3-wal"));
            let _ = fs::remove_file(self.0.with_extension("sqlite3-shm"));
        }
    }

    fn seed_pair(database: &Database) -> (i64, i64) {
        let connection = database.lock_connection().unwrap();
        let now = now_ms();
        connection
            .execute(
                "INSERT INTO source_tracks (
                    provider, provider_track_id, title, normalized_title, artists_json,
                    normalized_artists, created_at, updated_at
                 ) VALUES ('spotify', 'source-1', 'Song', 'song', '[\"Artist\"]', 'artist', ?1, ?1)",
                [now],
            )
            .unwrap();
        let source_track_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO library_tracks (
                    title, normalized_title, artists_json, normalized_artists, created_at, updated_at
                 ) VALUES ('Song', 'song', '[\"Artist\"]', 'artist', ?1, ?1)",
                [now],
            )
            .unwrap();
        let library_track_id = connection.last_insert_rowid();
        (source_track_id, library_track_id)
    }

    #[test]
    fn manual_confirmation_replaces_link_and_clears_matching_rejection() {
        let path = TestDatabasePath::new();
        let database = Database::open(path.0.clone()).unwrap();
        let (source_track_id, library_track_id) = seed_pair(&database);
        database
            .reject_match(source_track_id, library_track_id, Some("wrong version"))
            .unwrap();
        database
            .confirm_match(source_track_id, library_track_id)
            .unwrap();

        let link = database
            .persisted_track_link(source_track_id)
            .unwrap()
            .unwrap();
        assert_eq!(link.library_track_id, library_track_id);
        assert_eq!(link.method, "user");
        assert_eq!(link.confidence, 10_000);
        assert!(link.confirmed_by_user);
        assert!(
            database
                .rejected_library_track_ids(source_track_id)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn manual_rejection_removes_same_pair_link_and_persists() {
        let path = TestDatabasePath::new();
        let database = Database::open(path.0.clone()).unwrap();
        let (source_track_id, library_track_id) = seed_pair(&database);
        database
            .confirm_match(source_track_id, library_track_id)
            .unwrap();
        database
            .reject_match(source_track_id, library_track_id, None)
            .unwrap();

        assert!(
            database
                .persisted_track_link(source_track_id)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            database
                .rejected_library_track_ids(source_track_id)
                .unwrap(),
            HashSet::from([library_track_id])
        );
    }

    #[test]
    fn clearing_manual_decisions_preserves_non_user_links() {
        let path = TestDatabasePath::new();
        let database = Database::open(path.0.clone()).unwrap();
        let (source_track_id, library_track_id) = seed_pair(&database);
        {
            let connection = database.lock_connection().unwrap();
            let now = now_ms();
            connection
                .execute(
                    "INSERT INTO track_links (
                        source_track_id, library_track_id, method, confidence,
                        confirmed_by_user, created_at, updated_at
                     ) VALUES (?1, ?2, 'metadata', 9400, 0, ?3, ?3)",
                    params![source_track_id, library_track_id, now],
                )
                .unwrap();
        }

        database
            .clear_match_decision(source_track_id, None)
            .unwrap();
        assert_eq!(
            database
                .persisted_track_link(source_track_id)
                .unwrap()
                .unwrap()
                .method,
            "metadata"
        );
    }
}
