CREATE TABLE mirror_targets (
    id INTEGER PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    identity TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);

CREATE TABLE mirror_runs (
    id INTEGER PRIMARY KEY,
    target_id INTEGER NOT NULL REFERENCES mirror_targets(id) ON DELETE CASCADE,
    status TEXT NOT NULL CHECK (status IN ('running', 'succeeded', 'failed')),
    started_at INTEGER NOT NULL,
    finished_at INTEGER,
    copied INTEGER NOT NULL DEFAULT 0,
    unchanged INTEGER NOT NULL DEFAULT 0,
    removed INTEGER NOT NULL DEFAULT 0,
    error TEXT
);

CREATE INDEX idx_mirror_runs_target ON mirror_runs(target_id, id DESC);

CREATE TABLE mirror_entries (
    target_id INTEGER NOT NULL REFERENCES mirror_targets(id) ON DELETE CASCADE,
    relative_path TEXT NOT NULL,
    source_key TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    content_hash TEXT NOT NULL,
    PRIMARY KEY (target_id, relative_path)
);
