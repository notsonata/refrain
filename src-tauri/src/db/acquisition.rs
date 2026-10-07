use rusqlite::{OptionalExtension, Row, params};

use crate::{
    acquisition::rank_candidates,
    domain::{
        AcquisitionCandidate, AcquisitionJob, AcquisitionJobPage, StagingItem, StagingPage,
        TrackQuery,
    },
};

use super::{Database, DatabaseError, now_ms};

const MAX_PAGE_LIMIT: u32 = 500;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AcquisitionImportMetadata {
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub isrc: Option<String>,
    pub disc_number: Option<i64>,
    pub track_number: Option<i64>,
    pub release_year: Option<i64>,
    pub image_url: Option<String>,
}

impl Database {
    pub(crate) fn queue_missing_acquisition_jobs(
        &self,
        provider: &str,
    ) -> Result<Vec<AcquisitionJob>, DatabaseError> {
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        let now = now_ms();
        transaction.execute(
            "UPDATE acquisition_jobs AS job
             SET status = 'cancelled', finished_at = ?1, updated_at = ?1
             WHERE job.status = 'queued'
               AND NOT EXISTS (
                 SELECT 1
                 FROM track_links AS link
                 INNER JOIN collection_entries AS entry
                    ON entry.source_track_id = link.source_track_id
                 INNER JOIN source_collections AS collection
                    ON collection.id = entry.collection_id
                 LEFT JOIN source_collection_sync_rules AS rule
                    ON rule.collection_id = collection.id
                 LEFT JOIN source_track_sync_overrides AS override
                    ON override.collection_id = collection.id
                   AND override.source_track_id = entry.source_track_id
                 WHERE link.library_track_id = job.library_track_id
                   AND collection.is_accessible = 1
                   AND entry.item_type = 'track'
                   AND COALESCE(override.included, rule.default_included, 0) = 1
               )",
            [now],
        )?;
        transaction.execute(
            "UPDATE acquisition_jobs AS job
             SET provider = ?1,
                 status = 'queued',
                 stage = 'queued',
                 attempt = 1,
                 provider_job_id = NULL,
                 candidate_json = NULL,
                 candidates_json = NULL,
                 staging_path = NULL,
                 error_code = NULL,
                 error_message = NULL,
                 started_at = NULL,
                 finished_at = NULL,
                 updated_at = ?2
             WHERE job.status <> 'staged'
               AND (
                 job.provider <> ?1
                 OR job.status IN ('failed', 'cancelled')
               )
               AND EXISTS (
                 SELECT 1
                 FROM track_links AS link
                 INNER JOIN collection_entries AS entry
                    ON entry.source_track_id = link.source_track_id
                 INNER JOIN source_collections AS collection
                    ON collection.id = entry.collection_id
                 LEFT JOIN source_collection_sync_rules AS rule
                    ON rule.collection_id = collection.id
                 LEFT JOIN source_track_sync_overrides AS override
                    ON override.collection_id = collection.id
                   AND override.source_track_id = entry.source_track_id
                 WHERE link.library_track_id = job.library_track_id
                   AND collection.is_accessible = 1
                   AND entry.item_type = 'track'
                   AND COALESCE(override.included, rule.default_included, 0) = 1
               )
               AND NOT EXISTS (
                 SELECT 1 FROM local_files AS file
                 WHERE file.library_track_id = job.library_track_id
                   AND file.state = 'present'
               )",
            params![provider, now],
        )?;
        transaction.execute(
            "INSERT OR IGNORE INTO acquisition_jobs (
                library_track_id, provider, status, stage, attempt, created_at, updated_at
             )
             SELECT track.id, ?1, 'queued', 'queued', 1, ?2, ?2
             FROM library_tracks AS track
             WHERE EXISTS (
                 SELECT 1
                 FROM track_links AS link
                 INNER JOIN collection_entries AS entry
                    ON entry.source_track_id = link.source_track_id
                 INNER JOIN source_collections AS collection
                    ON collection.id = entry.collection_id
                 LEFT JOIN source_collection_sync_rules AS rule
                    ON rule.collection_id = collection.id
                 LEFT JOIN source_track_sync_overrides AS override
                    ON override.collection_id = collection.id
                   AND override.source_track_id = entry.source_track_id
                 WHERE link.library_track_id = track.id
                   AND collection.is_accessible = 1
                   AND entry.item_type = 'track'
                   AND COALESCE(override.included, rule.default_included, 0) = 1
             )
               AND NOT EXISTS (
                 SELECT 1 FROM local_files AS file
                 WHERE file.library_track_id = track.id
                   AND file.state = 'present'
             )",
            params![provider, now],
        )?;
        transaction.commit()?;
        drop(connection);
        self.acquisition_jobs_by_status("queued")
    }

    pub(crate) fn acquisition_track_query(
        &self,
        library_track_id: i64,
    ) -> Result<Option<TrackQuery>, DatabaseError> {
        self.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT id, title, artists_json, album, duration_ms, isrc,
                            version_kind, version_detail
                     FROM library_tracks
                     WHERE id = ?1",
                    [library_track_id],
                    |row| {
                        let artists_json = row.get::<_, String>(2)?;
                        Ok(TrackQuery {
                            library_track_id: row.get(0)?,
                            title: row.get(1)?,
                            artists: serde_json::from_str(&artists_json).unwrap_or_default(),
                            album: row.get(3)?,
                            duration_ms: row.get(4)?,
                            isrc: row.get(5)?,
                            version_kind: row.get(6)?,
                            version_detail: row.get(7)?,
                        })
                    },
                )
                .optional()
        })
    }

    pub(crate) fn acquisition_import_metadata(
        &self,
        library_track_id: i64,
    ) -> Result<Option<AcquisitionImportMetadata>, DatabaseError> {
        self.with_connection(|connection| {
            connection
                .query_row(
                    "SELECT
                        track.title,
                        track.artists_json,
                        track.album,
                        track.isrc,
                        track.disc_number,
                        track.track_number,
                        track.release_year,
                        COALESCE(
                            (
                                SELECT source.image_url
                                FROM source_tracks AS source
                                WHERE source.id = track.canonical_source_track_id
                                  AND source.image_url IS NOT NULL
                            ),
                            (
                                SELECT source.image_url
                                FROM track_links AS link
                                INNER JOIN source_tracks AS source
                                  ON source.id = link.source_track_id
                                WHERE link.library_track_id = track.id
                                  AND source.image_url IS NOT NULL
                                ORDER BY
                                  CASE WHEN source.provider = 'spotify' THEN 0 ELSE 1 END,
                                  source.id
                                LIMIT 1
                            )
                        )
                     FROM library_tracks AS track
                     WHERE track.id = ?1",
                    [library_track_id],
                    |row| {
                        let artists_json = row.get::<_, String>(1)?;
                        Ok(AcquisitionImportMetadata {
                            title: row.get(0)?,
                            artists: serde_json::from_str(&artists_json).unwrap_or_default(),
                            album: row.get(2)?,
                            isrc: row.get(3)?,
                            disc_number: row.get(4)?,
                            track_number: row.get(5)?,
                            release_year: row.get(6)?,
                            image_url: row.get(7)?,
                        })
                    },
                )
                .optional()
        })
    }

    pub fn acquisition_jobs_page(
        &self,
        offset: u32,
        limit: u32,
    ) -> Result<AcquisitionJobPage, DatabaseError> {
        let limit = limit.clamp(1, MAX_PAGE_LIMIT);
        self.with_connection(|connection| {
            let total =
                connection.query_row("SELECT COUNT(*) FROM acquisition_jobs", [], |row| {
                    row.get::<_, i64>(0)
                })? as usize;
            let mut statement = connection.prepare(&format!(
                "{} ORDER BY job.updated_at DESC, job.id DESC LIMIT ?1 OFFSET ?2",
                acquisition_job_select()
            ))?;
            let items = statement
                .query_map(
                    params![i64::from(limit), i64::from(offset)],
                    acquisition_job_from_row,
                )?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(AcquisitionJobPage {
                items,
                total,
                offset,
                limit,
            })
        })
    }

    pub fn staging_items_page(
        &self,
        offset: u32,
        limit: u32,
    ) -> Result<StagingPage, DatabaseError> {
        let limit = limit.clamp(1, MAX_PAGE_LIMIT);
        self.with_connection(|connection| {
            let total = connection.query_row(
                "SELECT COUNT(*)
                 FROM library_tracks AS track
                 WHERE EXISTS (
                     SELECT 1
                     FROM track_links AS link
                     INNER JOIN collection_entries AS entry
                       ON entry.source_track_id = link.source_track_id
                     INNER JOIN source_collections AS collection
                       ON collection.id = entry.collection_id
                     LEFT JOIN source_collection_sync_rules AS rule
                       ON rule.collection_id = collection.id
                     LEFT JOIN source_track_sync_overrides AS override
                       ON override.collection_id = collection.id
                      AND override.source_track_id = entry.source_track_id
                     WHERE link.library_track_id = track.id
                       AND collection.is_accessible = 1
                       AND entry.item_type = 'track'
                       AND COALESCE(override.included, rule.default_included, 0) = 1
                 )
                   AND NOT EXISTS (
                     SELECT 1 FROM local_files AS file
                     WHERE file.library_track_id = track.id
                       AND file.state = 'present'
                 )",
                [],
                |row| row.get::<_, i64>(0),
            )? as usize;

            let mut statement = connection.prepare(
                "SELECT
                    track.id,
                    job.id,
                    track.title,
                    track.artists_json,
                    track.album,
                    track.duration_ms,
                    job.provider,
                    job.provider_job_id,
                    COALESCE(job.stage,
                        CASE job.status
                            WHEN 'queued' THEN 'queued'
                            WHEN 'running' THEN 'downloading'
                            WHEN 'staged' THEN 'downloaded'
                            WHEN 'failed' THEN 'failed'
                            WHEN 'cancelled' THEN 'cancelled'
                            ELSE 'needsLocalCopy'
                        END,
                        'needsLocalCopy'
                    ),
                    job.status,
                    COALESCE(job.attempt, 0),
                    job.candidate_json,
                    job.candidates_json,
                    job.staging_path,
                    job.error_code,
                    job.error_message,
                    job.created_at,
                    job.started_at,
                    job.finished_at,
                    job.updated_at,
                    COALESCE(
                        (
                            SELECT source.image_url
                            FROM source_tracks AS source
                            WHERE source.id = track.canonical_source_track_id
                              AND source.image_url IS NOT NULL
                        ),
                        (
                            SELECT source.image_url
                            FROM track_links AS artwork_link
                            INNER JOIN source_tracks AS source
                              ON source.id = artwork_link.source_track_id
                            WHERE artwork_link.library_track_id = track.id
                              AND source.image_url IS NOT NULL
                            ORDER BY
                              CASE WHEN source.provider = 'spotify' THEN 0 ELSE 1 END,
                              source.id
                            LIMIT 1
                        )
                    ),
                    track.isrc
                 FROM library_tracks AS track
                 LEFT JOIN acquisition_jobs AS job
                   ON job.library_track_id = track.id
                 WHERE EXISTS (
                     SELECT 1
                     FROM track_links AS link
                     INNER JOIN collection_entries AS entry
                       ON entry.source_track_id = link.source_track_id
                     INNER JOIN source_collections AS collection
                       ON collection.id = entry.collection_id
                     LEFT JOIN source_collection_sync_rules AS rule
                       ON rule.collection_id = collection.id
                     LEFT JOIN source_track_sync_overrides AS override
                       ON override.collection_id = collection.id
                      AND override.source_track_id = entry.source_track_id
                     WHERE link.library_track_id = track.id
                       AND collection.is_accessible = 1
                       AND entry.item_type = 'track'
                       AND COALESCE(override.included, rule.default_included, 0) = 1
                 )
                   AND NOT EXISTS (
                     SELECT 1 FROM local_files AS file
                     WHERE file.library_track_id = track.id
                       AND file.state = 'present'
                 )
                 ORDER BY
                   CASE COALESCE(job.stage, 'needsLocalCopy')
                     WHEN 'needsResolution' THEN 0
                     WHEN 'failed' THEN 1
                     WHEN 'downloading' THEN 2
                     WHEN 'searching' THEN 3
                     WHEN 'queued' THEN 4
                     WHEN 'downloaded' THEN 5
                     WHEN 'needsLocalCopy' THEN 6
                     WHEN 'cancelled' THEN 7
                     ELSE 8
                   END,
                   COALESCE(job.updated_at, track.updated_at) DESC,
                   track.id DESC
                 LIMIT ?1 OFFSET ?2",
            )?;
            let items = statement
                .query_map(
                    params![i64::from(limit), i64::from(offset)],
                    staging_item_from_row,
                )?
                .collect::<Result<Vec<_>, _>>()?;
            Ok(StagingPage {
                items,
                total,
                offset,
                limit,
            })
        })
    }

    pub(crate) fn acquisition_job(
        &self,
        job_id: i64,
    ) -> Result<Option<AcquisitionJob>, DatabaseError> {
        self.with_connection(|connection| {
            connection
                .query_row(
                    &format!("{} WHERE job.id = ?1", acquisition_job_select()),
                    [job_id],
                    acquisition_job_from_row,
                )
                .optional()
        })
    }

    pub(crate) fn acquisition_jobs_by_status(
        &self,
        status: &str,
    ) -> Result<Vec<AcquisitionJob>, DatabaseError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(&format!(
                "{} WHERE job.status = ?1 ORDER BY job.id",
                acquisition_job_select()
            ))?;
            statement
                .query_map([status], acquisition_job_from_row)?
                .collect::<Result<Vec<_>, _>>()
        })
    }

    pub(crate) fn downloaded_acquisition_jobs_missing_local(
        &self,
    ) -> Result<Vec<AcquisitionJob>, DatabaseError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(&format!(
                "{} WHERE job.status = 'staged'
                     AND job.stage = 'downloaded'
                     AND NOT EXISTS (
                       SELECT 1
                       FROM local_files AS file
                       WHERE file.library_track_id = job.library_track_id
                         AND file.state = 'present'
                     )
                   ORDER BY job.updated_at, job.id",
                acquisition_job_select()
            ))?;
            statement
                .query_map([], acquisition_job_from_row)?
                .collect::<Result<Vec<_>, _>>()
        })
    }

    pub(crate) fn queue_acquisition_job(
        &self,
        library_track_id: i64,
        provider: &str,
    ) -> Result<Option<AcquisitionJob>, DatabaseError> {
        let now = now_ms();
        self.with_connection(|connection| {
            connection.execute(
                "INSERT INTO acquisition_jobs (
                    library_track_id, provider, status, stage, attempt, created_at, updated_at
                 )
                 SELECT track.id, ?2, 'queued', 'queued', 1, ?3, ?3
                 FROM library_tracks AS track
                 WHERE track.id = ?1
                   AND EXISTS (
                     SELECT 1
                     FROM track_links AS link
                     INNER JOIN collection_entries AS entry
                       ON entry.source_track_id = link.source_track_id
                     INNER JOIN source_collections AS collection
                       ON collection.id = entry.collection_id
                     LEFT JOIN source_collection_sync_rules AS rule
                       ON rule.collection_id = collection.id
                     LEFT JOIN source_track_sync_overrides AS override
                       ON override.collection_id = collection.id
                      AND override.source_track_id = entry.source_track_id
                     WHERE link.library_track_id = track.id
                       AND collection.is_accessible = 1
                       AND entry.item_type = 'track'
                       AND COALESCE(override.included, rule.default_included, 0) = 1
                   )
                   AND NOT EXISTS (
                     SELECT 1 FROM local_files AS file
                     WHERE file.library_track_id = track.id
                       AND file.state = 'present'
                   )
                 ON CONFLICT(library_track_id) DO UPDATE SET
                    provider = excluded.provider,
                    status = 'queued',
                    stage = 'queued',
                    attempt = 1,
                    provider_job_id = NULL,
                    candidate_json = NULL,
                    candidates_json = NULL,
                    staging_path = NULL,
                    error_code = NULL,
                    error_message = NULL,
                    started_at = NULL,
                    finished_at = NULL,
                    updated_at = excluded.updated_at",
                params![library_track_id, provider, now],
            )?;
            connection
                .query_row(
                    &format!(
                        "{} WHERE job.library_track_id = ?1",
                        acquisition_job_select()
                    ),
                    [library_track_id],
                    acquisition_job_from_row,
                )
                .optional()
        })
    }

    pub(crate) fn set_acquisition_stage(
        &self,
        job_id: i64,
        stage: &str,
    ) -> Result<(), DatabaseError> {
        self.with_connection(|connection| {
            connection.execute(
                "UPDATE acquisition_jobs SET stage = ?2, updated_at = ?3 WHERE id = ?1",
                params![job_id, stage, now_ms()],
            )?;
            Ok(())
        })
    }

    pub(crate) fn set_acquisition_provider(
        &self,
        job_id: i64,
        provider: &str,
    ) -> Result<(), DatabaseError> {
        self.with_connection(|connection| {
            connection.execute(
                "UPDATE acquisition_jobs SET provider = ?2, updated_at = ?3 WHERE id = ?1",
                params![job_id, provider, now_ms()],
            )?;
            Ok(())
        })
    }

    pub(crate) fn set_acquisition_recovery_error(
        &self,
        job_id: i64,
        error_code: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<(), DatabaseError> {
        self.with_connection(|connection| {
            connection.execute(
                "UPDATE acquisition_jobs
                 SET error_code = ?2, error_message = ?3, updated_at = ?4
                 WHERE id = ?1",
                params![job_id, error_code, error_message, now_ms()],
            )?;
            Ok(())
        })
    }

    pub(crate) fn require_acquisition_resolution(
        &self,
        job_id: i64,
        candidates: &[AcquisitionCandidate],
    ) -> Result<(), DatabaseError> {
        let candidates_json = serde_json::to_string(candidates)
            .map_err(|error| DatabaseError::InvalidState(error.to_string()))?;
        let now = now_ms();
        self.with_connection(|connection| {
            connection.execute(
                "UPDATE acquisition_jobs
                 SET status = 'failed', stage = 'needsResolution', candidates_json = ?2,
                     candidate_json = NULL, provider_job_id = NULL,
                     error_code = 'needsResolution',
                     error_message = 'Choose the best provider candidate before downloading.',
                     finished_at = ?3, updated_at = ?3
                 WHERE id = ?1",
                params![job_id, candidates_json, now],
            )?;
            Ok(())
        })
    }

    pub(crate) fn reject_acquisition_candidate(
        &self,
        job_id: i64,
        provider: &str,
        provider_token: &str,
    ) -> Result<Vec<AcquisitionCandidate>, DatabaseError> {
        let job = self.acquisition_job(job_id)?.ok_or_else(|| {
            DatabaseError::InvalidState(format!("acquisition job {job_id} not found"))
        })?;
        let job_provider = job.provider.clone();
        let mut candidates = job.candidates;
        candidates.retain(|candidate| {
            let candidate_provider = candidate.provider.as_deref().unwrap_or(&job_provider);
            candidate_provider != provider || candidate.provider_token != provider_token
        });
        let candidates_json = serde_json::to_string(&candidates)
            .map_err(|error| DatabaseError::InvalidState(error.to_string()))?;
        self.with_connection(|connection| {
            connection.execute(
                "UPDATE acquisition_jobs SET candidates_json = ?2, updated_at = ?3 WHERE id = ?1",
                params![job_id, candidates_json, now_ms()],
            )?;
            Ok(())
        })?;
        Ok(candidates)
    }

    pub(crate) fn failed_acquisition_jobs(&self) -> Result<Vec<AcquisitionJob>, DatabaseError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(&format!(
                "{} WHERE job.status = 'failed'
                 ORDER BY job.updated_at, job.id",
                acquisition_job_select()
            ))?;
            statement
                .query_map([], acquisition_job_from_row)?
                .collect::<Result<Vec<_>, _>>()
        })
    }

    pub(crate) fn active_acquisition_jobs(&self) -> Result<Vec<AcquisitionJob>, DatabaseError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(&format!(
                "{} WHERE job.status IN ('queued', 'running') ORDER BY job.id",
                acquisition_job_select()
            ))?;
            statement
                .query_map([], acquisition_job_from_row)?
                .collect::<Result<Vec<_>, _>>()
        })
    }

    pub(crate) fn clear_acquisition_jobs(&self) -> Result<usize, DatabaseError> {
        self.with_connection(|connection| connection.execute("DELETE FROM acquisition_jobs", []))
    }

    pub(crate) fn recover_interrupted_acquisition_jobs(&self) -> Result<usize, DatabaseError> {
        let now = now_ms();
        self.with_connection(|connection| {
            connection.execute(
                "UPDATE acquisition_jobs
                 SET status = 'failed',
                     stage = 'failed',
                     provider_job_id = NULL,
                     error_code = 'acquisitionInterrupted',
                     error_message = 'The previous acquisition was interrupted. Retry or sync to continue.',
                     finished_at = ?1,
                     updated_at = ?1
                 WHERE status = 'running'
                    OR stage IN ('searching', 'downloading')",
                [now],
            )
        })
    }

    pub(crate) fn begin_acquisition_attempt(
        &self,
        job_id: i64,
        attempt: i64,
        candidate: &AcquisitionCandidate,
        staging_path: &str,
    ) -> Result<(), DatabaseError> {
        let now = now_ms();
        let candidate_json = serde_json::to_string(candidate)
            .map_err(|error| DatabaseError::InvalidState(error.to_string()))?;
        self.with_connection(|connection| {
            connection.execute(
                "UPDATE acquisition_jobs
                 SET status = 'running', attempt = ?2, candidate_json = ?3,
                     stage = 'downloading', staging_path = ?4, provider_job_id = NULL,
                     error_code = NULL, error_message = NULL,
                     started_at = COALESCE(started_at, ?5), finished_at = NULL, updated_at = ?5
                 WHERE id = ?1",
                params![job_id, attempt, candidate_json, staging_path, now],
            )?;
            Ok(())
        })
    }

    pub(crate) fn set_acquisition_provider_job_id(
        &self,
        job_id: i64,
        provider_job_id: &str,
    ) -> Result<(), DatabaseError> {
        self.with_connection(|connection| {
            connection.execute(
                "UPDATE acquisition_jobs SET provider_job_id = ?2, updated_at = ?3 WHERE id = ?1",
                params![job_id, provider_job_id, now_ms()],
            )?;
            Ok(())
        })
    }

    pub(crate) fn update_acquisition_candidate(
        &self,
        job_id: i64,
        candidate: &AcquisitionCandidate,
    ) -> Result<(), DatabaseError> {
        let candidate_json = serde_json::to_string(candidate)
            .map_err(|error| DatabaseError::InvalidState(error.to_string()))?;
        self.with_connection(|connection| {
            connection.execute(
                "UPDATE acquisition_jobs SET candidate_json = ?2, updated_at = ?3 WHERE id = ?1",
                params![job_id, candidate_json, now_ms()],
            )?;
            Ok(())
        })
    }

    pub(crate) fn finish_acquisition_job(
        &self,
        job_id: i64,
        status: &str,
        error_code: Option<&str>,
        error_message: Option<&str>,
    ) -> Result<(), DatabaseError> {
        if !matches!(status, "staged" | "failed" | "cancelled") {
            return Err(DatabaseError::InvalidState(format!(
                "invalid terminal acquisition status {status}"
            )));
        }
        let now = now_ms();
        let stage = match status {
            "staged" => "downloaded",
            "failed" => "failed",
            "cancelled" => "cancelled",
            _ => unreachable!(),
        };
        self.with_connection(|connection| {
            connection.execute(
                "UPDATE acquisition_jobs
                 SET status = ?2, stage = ?3, error_code = ?4, error_message = ?5,
                     finished_at = ?6, updated_at = ?6
                 WHERE id = ?1",
                params![job_id, status, stage, error_code, error_message, now],
            )?;
            Ok(())
        })
    }
}

fn acquisition_job_select() -> &'static str {
    "SELECT
        job.id, job.library_track_id, track.title, track.artists_json,
        job.provider, job.provider_job_id, job.status, job.stage, job.attempt,
        job.candidate_json, job.candidates_json, job.staging_path, job.error_code, job.error_message,
        job.created_at, job.started_at, job.finished_at, job.updated_at
     FROM acquisition_jobs AS job
     JOIN library_tracks AS track ON track.id = job.library_track_id"
}

fn acquisition_job_from_row(row: &Row<'_>) -> Result<AcquisitionJob, rusqlite::Error> {
    let artists_json = row.get::<_, String>(3)?;
    let candidate_json = row.get::<_, Option<String>>(9)?;
    let candidates_json = row.get::<_, Option<String>>(10)?;
    Ok(AcquisitionJob {
        id: row.get(0)?,
        library_track_id: row.get(1)?,
        track_title: row.get(2)?,
        track_artists: serde_json::from_str(&artists_json).unwrap_or_default(),
        provider: row.get(4)?,
        provider_job_id: row.get(5)?,
        status: row.get(6)?,
        stage: row.get(7)?,
        attempt: row.get(8)?,
        candidate: candidate_json
            .as_deref()
            .and_then(|value| serde_json::from_str(value).ok()),
        candidates: candidates_json
            .as_deref()
            .and_then(|value| serde_json::from_str(value).ok())
            .unwrap_or_default(),
        staging_path: row.get(11)?,
        error_code: row.get(12)?,
        error_message: row.get(13)?,
        created_at: row.get(14)?,
        started_at: row.get(15)?,
        finished_at: row.get(16)?,
        updated_at: row.get(17)?,
    })
}

fn staging_item_from_row(row: &Row<'_>) -> Result<StagingItem, rusqlite::Error> {
    let artists_json = row.get::<_, String>(3)?;
    let artists = serde_json::from_str::<Vec<String>>(&artists_json).unwrap_or_default();
    let candidate_json = row.get::<_, Option<String>>(11)?;
    let candidates_json = row.get::<_, Option<String>>(12)?;
    let query = TrackQuery {
        library_track_id: row.get(0)?,
        title: row.get(2)?,
        artists: artists.clone(),
        album: row.get(4)?,
        duration_ms: row.get(5)?,
        isrc: row.get(21)?,
        version_kind: None,
        version_detail: None,
    };
    let candidate = candidate_json
        .as_deref()
        .and_then(|value| serde_json::from_str(value).ok())
        .and_then(|candidate| rank_candidates(&query, vec![candidate]).pop());
    let stored_candidates = candidates_json
        .as_deref()
        .and_then(|value| serde_json::from_str::<Vec<AcquisitionCandidate>>(value).ok())
        .unwrap_or_default();
    let candidates = if stored_candidates
        .iter()
        .all(|candidate| candidate.confidence.is_some())
    {
        stored_candidates
    } else {
        rank_candidates(&query, stored_candidates)
    };
    Ok(StagingItem {
        library_track_id: query.library_track_id,
        job_id: row.get(1)?,
        title: query.title,
        artists,
        album: query.album,
        duration_ms: query.duration_ms,
        image_url: row.get(20)?,
        provider: row.get(6)?,
        provider_job_id: row.get(7)?,
        stage: row.get(8)?,
        job_status: row.get(9)?,
        attempt: row.get(10)?,
        candidate,
        candidates,
        staging_path: row.get(13)?,
        error_code: row.get(14)?,
        error_message: row.get(15)?,
        bytes_transferred: None,
        total_bytes: None,
        created_at: row.get(16)?,
        started_at: row.get(17)?,
        finished_at: row.get(18)?,
        updated_at: row.get(19)?,
    })
}
