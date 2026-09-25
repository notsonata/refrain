use std::path::Path;

use rusqlite::{Connection, OptionalExtension, Row, Transaction, params};

use crate::{
    domain::{ReconciliationCounts, SyncRun, SyncRunPage},
    matching::normalize_comparison_text,
};

use super::{Database, DatabaseError, now_ms};

const MAX_SYNC_RUN_PAGE_LIMIT: u32 = 100;

#[derive(Debug)]
struct LocalTrackSeed {
    id: i64,
    path: String,
    content_hash: Option<String>,
    tag_title: Option<String>,
    tag_artists_json: Option<String>,
    tag_album: Option<String>,
    tag_isrc: Option<String>,
    duration_ms: Option<i64>,
}

#[derive(Debug)]
struct SourceTrackSeed {
    title: String,
    normalized_title: String,
    artists_json: String,
    normalized_artists: String,
    album: Option<String>,
    normalized_album: Option<String>,
    isrc: Option<String>,
    duration_ms: Option<i64>,
    disc_number: Option<i64>,
    track_number: Option<i64>,
    release_year: Option<i64>,
    explicit: Option<i64>,
    version_kind: Option<String>,
    version_detail: Option<String>,
}

impl Database {
    pub(crate) fn create_sync_run(&self, trigger: &str) -> Result<SyncRun, DatabaseError> {
        self.with_connection(|connection| {
            let now = now_ms();
            connection.execute(
                "INSERT INTO sync_runs (trigger, status, phase, started_at)
                 VALUES (?1, 'running', 'prepare', ?2)",
                params![trigger, now],
            )?;
            let id = connection.last_insert_rowid();
            sync_run_by_id(connection, id)?.ok_or(rusqlite::Error::QueryReturnedNoRows)
        })
    }

    pub fn sync_run(&self, id: i64) -> Result<Option<SyncRun>, DatabaseError> {
        self.with_connection(|connection| sync_run_by_id(connection, id))
    }

    pub fn sync_runs_page(&self, offset: u32, limit: u32) -> Result<SyncRunPage, DatabaseError> {
        let limit = limit.clamp(1, MAX_SYNC_RUN_PAGE_LIMIT);
        self.with_connection(|connection| {
            let total = connection.query_row("SELECT COUNT(*) FROM sync_runs", [], |row| {
                row.get::<_, i64>(0)
            })? as usize;
            let mut statement = connection.prepare(
                "SELECT
                    id, trigger, status, phase, started_at, finished_at,
                    source_added, source_removed, matched, missing, needs_review,
                    acquisition_failed, error_message
                 FROM sync_runs
                 ORDER BY started_at DESC, id DESC
                 LIMIT ?1 OFFSET ?2",
            )?;
            let items = statement
                .query_map(
                    params![i64::from(limit), i64::from(offset)],
                    sync_run_from_row,
                )?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(SyncRunPage {
                items,
                total,
                offset,
                limit,
            })
        })
    }

    pub(crate) fn update_sync_run_phase(&self, id: i64, phase: &str) -> Result<(), DatabaseError> {
        self.with_connection(|connection| {
            connection.execute(
                "UPDATE sync_runs SET phase = ?1 WHERE id = ?2 AND status = 'running'",
                params![phase, id],
            )?;
            Ok(())
        })
    }

    pub(crate) fn finish_sync_run(
        &self,
        id: i64,
        status: &str,
        counts: ReconciliationCounts,
        error_message: Option<&str>,
    ) -> Result<(), DatabaseError> {
        self.with_connection(|connection| {
            let final_phase = (status == "succeeded").then_some("complete");
            connection.execute(
                "UPDATE sync_runs
                 SET status = ?1,
                     phase = COALESCE(?2, phase),
                     finished_at = ?3,
                     matched = ?4,
                     missing = ?5,
                     needs_review = ?6,
                     error_message = ?7
                 WHERE id = ?8",
                params![
                    status,
                    final_phase,
                    now_ms(),
                    counts.matched,
                    counts.missing,
                    counts.needs_review,
                    error_message,
                    id,
                ],
            )?;
            Ok(())
        })
    }

    pub(crate) fn accessible_source_track_ids(&self) -> Result<Vec<i64>, DatabaseError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT DISTINCT track.id
                 FROM source_tracks AS track
                 INNER JOIN collection_entries AS entry
                    ON entry.source_track_id = track.id
                 INNER JOIN source_collections AS collection
                    ON collection.id = entry.collection_id
                 WHERE collection.is_accessible = 1
                   AND entry.item_type = 'track'
                 ORDER BY track.id",
            )?;
            statement
                .query_map([], |row| row.get::<_, i64>(0))?
                .collect::<Result<Vec<_>, _>>()
        })
    }

    pub(crate) fn ensure_library_tracks_for_present_local_files(
        &self,
    ) -> Result<usize, DatabaseError> {
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        let seeds = {
            let mut statement = transaction.prepare(
                "SELECT
                    id, path, content_hash, tag_title, tag_artists_json,
                    tag_album, tag_isrc, duration_ms
                 FROM local_files
                 WHERE state = 'present' AND library_track_id IS NULL
                 ORDER BY id",
            )?;
            statement
                .query_map([], |row| {
                    Ok(LocalTrackSeed {
                        id: row.get(0)?,
                        path: row.get(1)?,
                        content_hash: row.get(2)?,
                        tag_title: row.get(3)?,
                        tag_artists_json: row.get(4)?,
                        tag_album: row.get(5)?,
                        tag_isrc: row.get(6)?,
                        duration_ms: row.get(7)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?
        };

        let mut created = 0usize;
        for seed in seeds {
            let library_track_id = match existing_library_track_for_local_seed(&transaction, &seed)?
            {
                Some(id) => id,
                None => {
                    created += 1;
                    insert_library_track_from_local_seed(&transaction, &seed)?
                }
            };
            transaction.execute(
                "UPDATE local_files
                 SET library_track_id = ?1, updated_at = ?2
                 WHERE id = ?3",
                params![library_track_id, now_ms(), seed.id],
            )?;
        }

        normalize_preferred_present_files(&transaction)?;
        transaction.commit()?;
        Ok(created)
    }

    pub(crate) fn create_library_track_from_source(
        &self,
        source_track_id: i64,
    ) -> Result<i64, DatabaseError> {
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        if let Some(existing) = transaction
            .query_row(
                "SELECT id FROM library_tracks
                 WHERE canonical_source_track_id = ?1
                 ORDER BY id
                 LIMIT 1",
                [source_track_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
        {
            transaction.commit()?;
            return Ok(existing);
        }

        let seed = transaction.query_row(
            "SELECT
                title, normalized_title, artists_json, normalized_artists,
                album, normalized_album, isrc, duration_ms, disc_number,
                track_number, release_year, explicit, version_kind, version_detail
             FROM source_tracks
             WHERE id = ?1",
            [source_track_id],
            |row| {
                Ok(SourceTrackSeed {
                    title: row.get(0)?,
                    normalized_title: row.get(1)?,
                    artists_json: row.get(2)?,
                    normalized_artists: row.get(3)?,
                    album: row.get(4)?,
                    normalized_album: row.get(5)?,
                    isrc: row.get(6)?,
                    duration_ms: row.get(7)?,
                    disc_number: row.get(8)?,
                    track_number: row.get(9)?,
                    release_year: row.get(10)?,
                    explicit: row.get(11)?,
                    version_kind: row.get(12)?,
                    version_detail: row.get(13)?,
                })
            },
        )?;
        let now = now_ms();
        transaction.execute(
            "INSERT INTO library_tracks (
                canonical_source_track_id, title, normalized_title, artists_json,
                normalized_artists, album, normalized_album, isrc, duration_ms,
                disc_number, track_number, release_year, explicit, version_kind,
                version_detail, created_at, updated_at
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9,
                ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?16
             )",
            params![
                source_track_id,
                seed.title,
                seed.normalized_title,
                seed.artists_json,
                seed.normalized_artists,
                seed.album,
                seed.normalized_album,
                seed.isrc,
                seed.duration_ms,
                seed.disc_number,
                seed.track_number,
                seed.release_year,
                seed.explicit,
                seed.version_kind,
                seed.version_detail,
                now,
            ],
        )?;
        let id = transaction.last_insert_rowid();
        transaction.commit()?;
        Ok(id)
    }

    pub(crate) fn persist_track_link(
        &self,
        source_track_id: i64,
        library_track_id: i64,
        method: &str,
        confidence: i64,
    ) -> Result<(), DatabaseError> {
        if !matches!(
            method,
            "existing" | "isrc" | "metadata" | "acquisition" | "user"
        ) {
            return Err(DatabaseError::InvalidState(format!(
                "unsupported track link method: {method}"
            )));
        }
        if !(0..=10_000).contains(&confidence) {
            return Err(DatabaseError::InvalidState(
                "track-link confidence must be between 0 and 10000".into(),
            ));
        }
        self.with_connection(|connection| {
            let now = now_ms();
            connection.execute(
                "INSERT INTO track_links (
                    source_track_id, library_track_id, method, confidence,
                    confirmed_by_user, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, 0, ?5, ?5)
                 ON CONFLICT(source_track_id) DO UPDATE SET
                    library_track_id = excluded.library_track_id,
                    method = excluded.method,
                    confidence = excluded.confidence,
                    updated_at = excluded.updated_at
                 WHERE track_links.confirmed_by_user = 0",
                params![source_track_id, library_track_id, method, confidence, now],
            )?;
            Ok(())
        })
    }

    pub(crate) fn library_track_has_preferred_present_file(
        &self,
        library_track_id: i64,
    ) -> Result<bool, DatabaseError> {
        self.with_connection(|connection| {
            connection.query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM local_files
                    WHERE library_track_id = ?1
                      AND state = 'present'
                      AND is_preferred = 1
                 )",
                [library_track_id],
                |row| row.get::<_, i64>(0).map(|value| value != 0),
            )
        })
    }

    #[cfg(test)]
    pub(crate) fn library_track_count(&self) -> Result<i64, DatabaseError> {
        self.with_connection(|connection| {
            connection.query_row("SELECT COUNT(*) FROM library_tracks", [], |row| row.get(0))
        })
    }
}

fn sync_run_by_id(connection: &Connection, id: i64) -> Result<Option<SyncRun>, rusqlite::Error> {
    connection
        .query_row(
            "SELECT
                id, trigger, status, phase, started_at, finished_at,
                source_added, source_removed, matched, missing, needs_review,
                acquisition_failed, error_message
             FROM sync_runs
             WHERE id = ?1",
            [id],
            sync_run_from_row,
        )
        .optional()
}

fn sync_run_from_row(row: &Row<'_>) -> Result<SyncRun, rusqlite::Error> {
    Ok(SyncRun {
        id: row.get(0)?,
        trigger: row.get(1)?,
        status: row.get(2)?,
        phase: row.get(3)?,
        started_at: row.get(4)?,
        finished_at: row.get(5)?,
        source_added: row.get(6)?,
        source_removed: row.get(7)?,
        matched: row.get(8)?,
        missing: row.get(9)?,
        needs_review: row.get(10)?,
        acquisition_failed: row.get(11)?,
        error_message: row.get(12)?,
    })
}

fn existing_library_track_for_local_seed(
    transaction: &Transaction<'_>,
    seed: &LocalTrackSeed,
) -> Result<Option<i64>, rusqlite::Error> {
    if let Some(hash) = seed
        .content_hash
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        if let Some(id) = transaction
            .query_row(
                "SELECT library_track_id
                 FROM local_files
                 WHERE content_hash = ?1
                   AND library_track_id IS NOT NULL
                   AND state = 'present'
                 ORDER BY is_preferred DESC, id
                 LIMIT 1",
                [hash],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
        {
            return Ok(Some(id));
        }
    }

    if let Some(isrc) = seed
        .tag_isrc
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        return transaction
            .query_row(
                "SELECT id
                 FROM library_tracks
                 WHERE isrc = ?1 COLLATE NOCASE
                 ORDER BY id
                 LIMIT 1",
                [isrc],
                |row| row.get::<_, i64>(0),
            )
            .optional();
    }

    Ok(None)
}

fn insert_library_track_from_local_seed(
    transaction: &Transaction<'_>,
    seed: &LocalTrackSeed,
) -> Result<i64, rusqlite::Error> {
    let fallback_title = Path::new(&seed.path)
        .file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("Unknown Track");
    let title = seed
        .tag_title
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(fallback_title)
        .to_owned();
    let artists = seed
        .tag_artists_json
        .as_deref()
        .and_then(|value| serde_json::from_str::<Vec<String>>(value).ok())
        .unwrap_or_default();
    let artists_json = serde_json::to_string(&artists).unwrap_or_else(|_| "[]".to_owned());
    let normalized_artists = artists
        .iter()
        .map(|artist| normalize_comparison_text(artist))
        .filter(|artist| !artist.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let normalized_title = normalize_comparison_text(&title);
    let normalized_album = seed
        .tag_album
        .as_deref()
        .map(normalize_comparison_text)
        .filter(|value| !value.is_empty());
    let now = now_ms();
    transaction.execute(
        "INSERT INTO library_tracks (
            title, normalized_title, artists_json, normalized_artists,
            album, normalized_album, isrc, duration_ms, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
        params![
            title,
            normalized_title,
            artists_json,
            normalized_artists,
            seed.tag_album.as_deref(),
            normalized_album,
            seed.tag_isrc.as_deref(),
            seed.duration_ms,
            now,
        ],
    )?;
    Ok(transaction.last_insert_rowid())
}

fn normalize_preferred_present_files(transaction: &Transaction<'_>) -> Result<(), rusqlite::Error> {
    let library_track_ids = {
        let mut statement = transaction.prepare(
            "SELECT DISTINCT library_track_id
             FROM local_files
             WHERE state = 'present' AND library_track_id IS NOT NULL
             ORDER BY library_track_id",
        )?;
        statement
            .query_map([], |row| row.get::<_, i64>(0))?
            .collect::<Result<Vec<_>, _>>()?
    };

    for library_track_id in library_track_ids {
        let preferred_id = transaction.query_row(
            "SELECT id
             FROM local_files
             WHERE library_track_id = ?1 AND state = 'present'
             ORDER BY is_preferred DESC, id
             LIMIT 1",
            [library_track_id],
            |row| row.get::<_, i64>(0),
        )?;
        transaction.execute(
            "UPDATE local_files
             SET is_preferred = CASE WHEN id = ?1 THEN 1 ELSE 0 END,
                 updated_at = ?2
             WHERE library_track_id = ?3
               AND is_preferred != CASE WHEN id = ?1 THEN 1 ELSE 0 END",
            params![preferred_id, now_ms(), library_track_id],
        )?;
    }
    Ok(())
}
