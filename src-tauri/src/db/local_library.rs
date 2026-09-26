use rusqlite::{OptionalExtension, Row, params};

use crate::domain::{
    LibraryTrackFileSummary, LibraryTrackPage, LibraryTrackRow, LocalFile, LocalFilePage,
    LocalLibraryOverview, SpotifyMembership,
};

use super::{Database, DatabaseError, now_ms};

const MAX_PAGE_LIMIT: u32 = 500;

#[derive(Debug, Clone)]
pub(crate) struct LocalFileWrite {
    pub path: String,
    pub state: String,
    pub format: Option<String>,
    pub file_size: i64,
    pub modified_at: i64,
    pub duration_ms: Option<i64>,
    pub bitrate: Option<i64>,
    pub sample_rate: Option<i64>,
    pub channels: Option<i64>,
    pub content_hash: Option<String>,
    pub tag_title: Option<String>,
    pub tag_artists: Vec<String>,
    pub tag_album: Option<String>,
    pub tag_isrc: Option<String>,
    pub artwork_path: Option<String>,
    pub artwork_mime: Option<String>,
    pub scan_error: Option<String>,
}

impl Database {
    pub fn library_tracks_page(
        &self,
        offset: u32,
        limit: u32,
    ) -> Result<LibraryTrackPage, DatabaseError> {
        let limit = limit.clamp(1, MAX_PAGE_LIMIT);
        self.with_connection(|connection| {
            let total = connection.query_row(
                "SELECT COUNT(*)
                 FROM library_tracks AS track
                 WHERE EXISTS (
                    SELECT 1 FROM local_files AS file
                    WHERE file.library_track_id = track.id AND file.state = 'present'
                 )",
                [],
                |row| row.get::<_, i64>(0),
            )? as usize;
            let mut statement = connection.prepare(
                "SELECT
                    track.id,
                    track.title,
                    track.artists_json,
                    track.album,
                    track.release_year,
                    track.duration_ms,
                    track.explicit,
                    (SELECT COUNT(*) FROM track_links AS link
                     WHERE link.library_track_id = track.id),
                    (SELECT job.status FROM acquisition_jobs AS job
                     WHERE job.library_track_id = track.id
                     ORDER BY job.updated_at DESC, job.id DESC
                     LIMIT 1),
                    (SELECT COUNT(*) FROM local_files AS file
                     WHERE file.library_track_id = track.id),
                    (SELECT COUNT(*) FROM local_files AS file
                     WHERE file.library_track_id = track.id AND file.state = 'present'),
                    (SELECT COUNT(*) FROM local_files AS file
                     WHERE file.library_track_id = track.id AND file.state = 'missing'),
                    (SELECT COUNT(*) FROM local_files AS file
                     WHERE file.library_track_id = track.id AND file.state = 'invalid'),
                    preferred.id,
                    preferred.path,
                    preferred.ownership,
                    preferred.state,
                    preferred.format,
                    preferred.artwork_path,
                    preferred.artwork_mime
                 FROM library_tracks AS track
                 LEFT JOIN local_files AS preferred
                   ON preferred.id = (
                        SELECT file.id
                        FROM local_files AS file
                        WHERE file.library_track_id = track.id
                          AND file.state = 'present'
                        ORDER BY file.is_preferred DESC, file.id
                        LIMIT 1
                   )
                 WHERE EXISTS (
                    SELECT 1 FROM local_files AS present_file
                    WHERE present_file.library_track_id = track.id
                      AND present_file.state = 'present'
                 )
                 ORDER BY track.normalized_artists, track.normalized_title, track.id
                 LIMIT ?1 OFFSET ?2",
            )?;
            let mut items = statement
                .query_map(
                    params![i64::from(limit), i64::from(offset)],
                    library_track_row_from_row,
                )?
                .collect::<Result<Vec<_>, _>>()?;
            for item in &mut items {
                item.spotify_memberships =
                    spotify_memberships_for_library_track(connection, item.id)?;
            }
            Ok(LibraryTrackPage {
                items,
                total,
                offset,
                limit,
            })
        })
    }

    pub(crate) fn local_files_snapshot(&self) -> Result<Vec<LocalFile>, DatabaseError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT
                    id, library_track_id, path, ownership, is_preferred, state, format,
                    file_size, modified_at, duration_ms, bitrate, sample_rate, channels,
                    content_hash, tag_title, tag_artists_json, tag_album, tag_isrc,
                    artwork_path, artwork_mime, scan_error
                 FROM local_files
                 ORDER BY id",
            )?;
            let rows = statement.query_map([], local_file_from_row)?;
            rows.collect::<Result<Vec<_>, _>>()
        })
    }

    pub fn local_files_page(
        &self,
        offset: u32,
        limit: u32,
    ) -> Result<LocalFilePage, DatabaseError> {
        let limit = limit.clamp(1, MAX_PAGE_LIMIT);
        self.with_connection(|connection| {
            let total = connection.query_row("SELECT COUNT(*) FROM local_files", [], |row| {
                row.get::<_, i64>(0)
            })? as usize;
            let mut statement = connection.prepare(
                "SELECT
                    id, library_track_id, path, ownership, is_preferred, state, format,
                    file_size, modified_at, duration_ms, bitrate, sample_rate, channels,
                    content_hash, tag_title, tag_artists_json, tag_album, tag_isrc,
                    artwork_path, artwork_mime, scan_error
                 FROM local_files
                 ORDER BY lower(path), id
                 LIMIT ?1 OFFSET ?2",
            )?;
            let rows = statement.query_map(
                params![i64::from(limit), i64::from(offset)],
                local_file_from_row,
            )?;
            let items = rows.collect::<Result<Vec<_>, _>>()?;
            Ok(LocalFilePage {
                items,
                total,
                offset,
                limit,
            })
        })
    }

    pub fn local_library_overview(&self) -> Result<LocalLibraryOverview, DatabaseError> {
        self.with_connection(|connection| {
            let mut overview = LocalLibraryOverview {
                total: 0,
                present: 0,
                missing: 0,
                invalid: 0,
            };
            let mut statement =
                connection.prepare("SELECT state, COUNT(*) FROM local_files GROUP BY state")?;
            let rows = statement.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize))
            })?;
            for row in rows {
                let (state, count) = row?;
                overview.total += count;
                match state.as_str() {
                    "present" => overview.present += count,
                    "missing" => overview.missing += count,
                    "invalid" => overview.invalid += count,
                    _ => {}
                }
            }
            Ok(overview)
        })
    }

    pub(crate) fn insert_local_file(&self, value: &LocalFileWrite) -> Result<i64, DatabaseError> {
        self.with_connection(|connection| {
            let now = now_ms();
            let artists_json = serde_json::to_string(&value.tag_artists)
                .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
            connection.execute(
                "INSERT INTO local_files (
                    path, ownership, state, format, file_size, modified_at, duration_ms,
                    bitrate, sample_rate, channels, content_hash, tag_title, tag_artists_json,
                    tag_album, tag_isrc, artwork_path, artwork_mime, scan_error, created_at, updated_at
                 ) VALUES (
                    ?1, 'external', ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?18
                 )",
                params![
                    value.path,
                    value.state,
                    value.format,
                    value.file_size,
                    value.modified_at,
                    value.duration_ms,
                    value.bitrate,
                    value.sample_rate,
                    value.channels,
                    value.content_hash,
                    value.tag_title,
                    artists_json,
                    value.tag_album,
                    value.tag_isrc,
                    value.artwork_path,
                    value.artwork_mime,
                    value.scan_error,
                    now,
                ],
            )?;
            Ok(connection.last_insert_rowid())
        })
    }

    pub(crate) fn update_local_file_scan(
        &self,
        id: i64,
        value: &LocalFileWrite,
    ) -> Result<(), DatabaseError> {
        self.with_connection(|connection| {
            let artists_json = serde_json::to_string(&value.tag_artists)
                .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))?;
            connection.execute(
                "UPDATE local_files
                 SET path = ?1,
                     state = ?2,
                     format = ?3,
                     file_size = ?4,
                     modified_at = ?5,
                     duration_ms = ?6,
                     bitrate = ?7,
                     sample_rate = ?8,
                     channels = ?9,
                     content_hash = ?10,
                     tag_title = ?11,
                     tag_artists_json = ?12,
                     tag_album = ?13,
                     tag_isrc = ?14,
                     artwork_path = ?15,
                     artwork_mime = ?16,
                     scan_error = ?17,
                     updated_at = ?18
                 WHERE id = ?19",
                params![
                    value.path,
                    value.state,
                    value.format,
                    value.file_size,
                    value.modified_at,
                    value.duration_ms,
                    value.bitrate,
                    value.sample_rate,
                    value.channels,
                    value.content_hash,
                    value.tag_title,
                    artists_json,
                    value.tag_album,
                    value.tag_isrc,
                    value.artwork_path,
                    value.artwork_mime,
                    value.scan_error,
                    now_ms(),
                    id,
                ],
            )?;
            Ok(())
        })
    }

    pub(crate) fn mark_local_files_missing(&self, ids: &[i64]) -> Result<usize, DatabaseError> {
        if ids.is_empty() {
            return Ok(0);
        }
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        let now = now_ms();
        let mut changed = 0usize;
        for id in ids {
            changed += transaction.execute(
                "UPDATE local_files
                 SET state = 'missing', scan_error = NULL, updated_at = ?1
                 WHERE id = ?2 AND state != 'missing'",
                params![now, id],
            )?;
        }
        transaction.commit()?;
        Ok(changed)
    }

    pub(crate) fn local_file(&self, id: i64) -> Result<Option<LocalFile>, DatabaseError> {
        self.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT
                        id, library_track_id, path, ownership, is_preferred, state, format,
                        file_size, modified_at, duration_ms, bitrate, sample_rate, channels,
                        content_hash, tag_title, tag_artists_json, tag_album, tag_isrc,
                        artwork_path, artwork_mime, scan_error
                     FROM local_files
                     WHERE id = ?1",
                    [id],
                    local_file_from_row,
                )
                .optional()
        })
    }

    pub(crate) fn local_files_for_library_track(
        &self,
        library_track_id: i64,
    ) -> Result<Vec<LocalFile>, DatabaseError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT
                    id, library_track_id, path, ownership, is_preferred, state, format,
                    file_size, modified_at, duration_ms, bitrate, sample_rate, channels,
                    content_hash, tag_title, tag_artists_json, tag_album, tag_isrc,
                    artwork_path, artwork_mime, scan_error
                 FROM local_files
                 WHERE library_track_id = ?1
                 ORDER BY is_preferred DESC, state = 'present' DESC, lower(path), id",
            )?;
            statement
                .query_map([library_track_id], local_file_from_row)?
                .collect::<Result<Vec<_>, _>>()
        })
    }

    pub(crate) fn update_local_file_hash(&self, id: i64, hash: &str) -> Result<(), DatabaseError> {
        self.with_connection(|connection| {
            connection.execute(
                "UPDATE local_files SET content_hash = ?1, updated_at = ?2 WHERE id = ?3",
                params![hash, now_ms(), id],
            )?;
            Ok(())
        })
    }

    pub fn set_preferred_local_file(&self, id: i64) -> Result<(), DatabaseError> {
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        let library_track_id = transaction
            .query_row(
                "SELECT library_track_id
                 FROM local_files
                 WHERE id = ?1 AND state = 'present'",
                [id],
                |row| row.get::<_, Option<i64>>(0),
            )
            .optional()?
            .flatten()
            .ok_or_else(|| {
                DatabaseError::InvalidState(
                    "preferred local file must be present and linked to a library track".into(),
                )
            })?;
        transaction.execute(
            "UPDATE local_files SET is_preferred = 0, updated_at = ?1 WHERE library_track_id = ?2",
            params![now_ms(), library_track_id],
        )?;
        transaction.execute(
            "UPDATE local_files SET is_preferred = 1, updated_at = ?1 WHERE id = ?2",
            params![now_ms(), id],
        )?;
        transaction.commit()?;
        Ok(())
    }
}

fn local_file_from_row(row: &Row<'_>) -> Result<LocalFile, rusqlite::Error> {
    let artists_json = row
        .get::<_, Option<String>>(15)?
        .unwrap_or_else(|| "[]".to_owned());
    let tag_artists = serde_json::from_str::<Vec<String>>(&artists_json).unwrap_or_default();
    Ok(LocalFile {
        id: row.get(0)?,
        library_track_id: row.get(1)?,
        path: row.get(2)?,
        ownership: row.get(3)?,
        is_preferred: row.get::<_, i64>(4)? != 0,
        state: row.get(5)?,
        format: row.get(6)?,
        file_size: row.get(7)?,
        modified_at: row.get(8)?,
        duration_ms: row.get(9)?,
        bitrate: row.get(10)?,
        sample_rate: row.get(11)?,
        channels: row.get(12)?,
        content_hash: row.get(13)?,
        tag_title: row.get(14)?,
        tag_artists,
        tag_album: row.get(16)?,
        tag_isrc: row.get(17)?,
        artwork_path: row.get(18)?,
        artwork_mime: row.get(19)?,
        scan_error: row.get(20)?,
    })
}

fn library_track_row_from_row(row: &Row<'_>) -> Result<LibraryTrackRow, rusqlite::Error> {
    let artists_json = row.get::<_, String>(2)?;
    let preferred_file = row
        .get::<_, Option<i64>>(13)?
        .map(|id| -> Result<LibraryTrackFileSummary, rusqlite::Error> {
            Ok(LibraryTrackFileSummary {
                id,
                path: row.get(14)?,
                ownership: row.get(15)?,
                state: row.get(16)?,
                format: row.get(17)?,
                artwork_path: row.get(18)?,
                artwork_mime: row.get(19)?,
            })
        })
        .transpose()?;
    Ok(LibraryTrackRow {
        id: row.get(0)?,
        title: row.get(1)?,
        artists: serde_json::from_str(&artists_json).unwrap_or_default(),
        album: row.get(3)?,
        release_year: row.get(4)?,
        duration_ms: row.get(5)?,
        explicit: row.get::<_, Option<i64>>(6)?.map(|value| value != 0),
        source_track_count: row.get::<_, i64>(7)? as usize,
        acquisition_status: row.get(8)?,
        local_file_count: row.get::<_, i64>(9)? as usize,
        present_file_count: row.get::<_, i64>(10)? as usize,
        missing_file_count: row.get::<_, i64>(11)? as usize,
        invalid_file_count: row.get::<_, i64>(12)? as usize,
        preferred_file,
        spotify_memberships: Vec::new(),
    })
}

fn spotify_memberships_for_library_track(
    connection: &rusqlite::Connection,
    library_track_id: i64,
) -> Result<Vec<SpotifyMembership>, rusqlite::Error> {
    let mut statement = connection.prepare(
        "SELECT DISTINCT collection.kind, collection.name
         FROM track_links AS link
         INNER JOIN collection_entries AS entry
            ON entry.source_track_id = link.source_track_id
         INNER JOIN source_collections AS collection
            ON collection.id = entry.collection_id
         WHERE link.library_track_id = ?1
           AND collection.is_accessible = 1
           AND entry.item_type = 'track'
         ORDER BY
            CASE collection.kind
                WHEN 'liked_songs' THEN 0
                WHEN 'saved_album' THEN 1
                WHEN 'playlist' THEN 2
                ELSE 3
            END,
            lower(collection.name)",
    )?;
    statement
        .query_map([library_track_id], |row| {
            Ok(SpotifyMembership {
                kind: row.get(0)?,
                name: row.get(1)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use super::*;

    static NEXT_DATABASE_ID: AtomicU64 = AtomicU64::new(0);

    struct TestDatabasePath(PathBuf);

    impl TestDatabasePath {
        fn new() -> Self {
            let id = NEXT_DATABASE_ID.fetch_add(1, Ordering::Relaxed);
            Self(std::env::temp_dir().join(format!(
                "refrain-local-library-db-{}-{id}.sqlite3",
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

    #[test]
    fn preferred_file_selection_keeps_one_present_file_per_library_track() {
        let path = TestDatabasePath::new();
        let database = Database::open(path.0.clone()).unwrap();
        let connection = database.lock_connection().unwrap();
        let now = now_ms();
        connection
            .execute(
                "INSERT INTO library_tracks (
                title, normalized_title, artists_json, normalized_artists, created_at, updated_at
             ) VALUES ('Track', 'track', '[\"Artist\"]', 'artist', ?1, ?1)",
                [now],
            )
            .unwrap();
        let library_track_id = connection.last_insert_rowid();
        connection.execute(
            "INSERT INTO local_files (
                library_track_id, path, ownership, state, file_size, modified_at, created_at, updated_at
             ) VALUES
                (?1, '/music/a.flac', 'external', 'present', 100, 1, ?2, ?2),
                (?1, '/music/b.flac', 'external', 'present', 100, 1, ?2, ?2)",
            params![library_track_id, now],
        ).unwrap();
        let ids = {
            let mut statement = connection
                .prepare("SELECT id FROM local_files ORDER BY id")
                .unwrap();
            statement
                .query_map([], |row| row.get::<_, i64>(0))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap()
        };
        drop(connection);

        database.set_preferred_local_file(ids[0]).unwrap();
        database.set_preferred_local_file(ids[1]).unwrap();

        let files = database.local_files_page(0, 10).unwrap().items;
        assert!(
            !files
                .iter()
                .find(|file| file.id == ids[0])
                .unwrap()
                .is_preferred
        );
        assert!(
            files
                .iter()
                .find(|file| file.id == ids[1])
                .unwrap()
                .is_preferred
        );
    }
}
