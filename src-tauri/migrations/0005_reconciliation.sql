CREATE TABLE sync_runs (
    id                  INTEGER PRIMARY KEY,
    trigger             TEXT NOT NULL CHECK (trigger IN ('manual', 'startup', 'scheduled')),
    status              TEXT NOT NULL CHECK (status IN ('running', 'succeeded', 'partial', 'failed', 'cancelled')),
    phase               TEXT NULL,
    started_at          INTEGER NOT NULL,
    finished_at         INTEGER NULL,
    source_added        INTEGER NOT NULL DEFAULT 0,
    source_removed      INTEGER NOT NULL DEFAULT 0,
    matched             INTEGER NOT NULL DEFAULT 0,
    missing             INTEGER NOT NULL DEFAULT 0,
    needs_review        INTEGER NOT NULL DEFAULT 0,
    acquisition_failed  INTEGER NOT NULL DEFAULT 0,
    error_message       TEXT NULL
);

CREATE INDEX idx_sync_runs_started_at ON sync_runs(started_at DESC);
