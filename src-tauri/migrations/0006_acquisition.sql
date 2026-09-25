CREATE TABLE acquisition_jobs (
    id                    INTEGER PRIMARY KEY,
    library_track_id      INTEGER NOT NULL REFERENCES library_tracks(id),
    provider              TEXT NOT NULL,
    provider_job_id       TEXT NULL,
    status                TEXT NOT NULL CHECK (status IN ('queued', 'running', 'staged', 'failed', 'cancelled')),
    attempt               INTEGER NOT NULL DEFAULT 1,
    candidate_json        TEXT NULL,
    staging_path          TEXT NULL,
    error_code            TEXT NULL,
    error_message         TEXT NULL,
    created_at            INTEGER NOT NULL,
    started_at            INTEGER NULL,
    finished_at           INTEGER NULL,
    updated_at            INTEGER NOT NULL,
    UNIQUE(library_track_id)
);

CREATE INDEX idx_acquisition_jobs_library_track_status
    ON acquisition_jobs(library_track_id, status);
CREATE INDEX idx_acquisition_jobs_status_updated
    ON acquisition_jobs(status, updated_at DESC);
