CREATE TABLE mirror_runs_new (
    id INTEGER PRIMARY KEY,
    target_id INTEGER NOT NULL REFERENCES mirror_targets(id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK (status IN ('running', 'succeeded', 'failed', 'cancelled')),
    started_at INTEGER NOT NULL,
    finished_at INTEGER,
    copied INTEGER NOT NULL DEFAULT 0,
    unchanged INTEGER NOT NULL DEFAULT 0,
    removed INTEGER NOT NULL DEFAULT 0,
    error TEXT
);

INSERT INTO mirror_runs_new SELECT * FROM mirror_runs;
DROP TABLE mirror_runs;
ALTER TABLE mirror_runs_new RENAME TO mirror_runs;
CREATE INDEX idx_mirror_runs_target ON mirror_runs(target_id, id DESC);
