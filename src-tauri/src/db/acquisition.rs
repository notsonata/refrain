use rusqlite::{OptionalExtension, Row, params};

use crate::domain::{AcquisitionCandidate, AcquisitionJob, AcquisitionJobPage, TrackQuery};

use super::{Database, DatabaseError, now_ms};

const MAX_PAGE_LIMIT: u32 = 500;

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
            "INSERT OR IGNORE INTO acquisition_jobs (
                library_track_id, provider, status, attempt, created_at, updated_at
             )
             SELECT track.id, ?1, 'queued', 1, ?2, ?2
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
                     staging_path = ?4, provider_job_id = NULL,
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
        self.with_connection(|connection| {
            connection.execute(
                "UPDATE acquisition_jobs
                 SET status = ?2, error_code = ?3, error_message = ?4,
                     finished_at = ?5, updated_at = ?5
                 WHERE id = ?1",
                params![job_id, status, error_code, error_message, now],
            )?;
            Ok(())
        })
    }
}

fn acquisition_job_select() -> &'static str {
    "SELECT
        job.id, job.library_track_id, track.title, track.artists_json,
        job.provider, job.provider_job_id, job.status, job.attempt,
        job.candidate_json, job.staging_path, job.error_code, job.error_message,
        job.created_at, job.started_at, job.finished_at, job.updated_at
     FROM acquisition_jobs AS job
     JOIN library_tracks AS track ON track.id = job.library_track_id"
}

fn acquisition_job_from_row(row: &Row<'_>) -> Result<AcquisitionJob, rusqlite::Error> {
    let artists_json = row.get::<_, String>(3)?;
    let candidate_json = row.get::<_, Option<String>>(8)?;
    Ok(AcquisitionJob {
        id: row.get(0)?,
        library_track_id: row.get(1)?,
        track_title: row.get(2)?,
        track_artists: serde_json::from_str(&artists_json).unwrap_or_default(),
        provider: row.get(4)?,
        provider_job_id: row.get(5)?,
        status: row.get(6)?,
        attempt: row.get(7)?,
        candidate: candidate_json
            .as_deref()
            .and_then(|value| serde_json::from_str(value).ok()),
        staging_path: row.get(9)?,
        error_code: row.get(10)?,
        error_message: row.get(11)?,
        created_at: row.get(12)?,
        started_at: row.get(13)?,
        finished_at: row.get(14)?,
        updated_at: row.get(15)?,
    })
}
