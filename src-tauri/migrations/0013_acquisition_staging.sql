ALTER TABLE acquisition_jobs
ADD COLUMN stage TEXT NULL;

ALTER TABLE acquisition_jobs
ADD COLUMN candidates_json TEXT NULL;

UPDATE acquisition_jobs
SET stage = CASE status
    WHEN 'queued' THEN 'queued'
    WHEN 'running' THEN 'downloading'
    WHEN 'staged' THEN 'downloaded'
    WHEN 'failed' THEN 'failed'
    WHEN 'cancelled' THEN 'cancelled'
    ELSE NULL
END
WHERE stage IS NULL;
