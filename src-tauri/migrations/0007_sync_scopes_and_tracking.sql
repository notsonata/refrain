ALTER TABLE sync_runs
ADD COLUMN scope TEXT NOT NULL DEFAULT 'legacy'
CHECK (scope IN ('legacy', 'local', 'spotify'));

CREATE INDEX idx_sync_runs_scope_started_at
ON sync_runs(scope, started_at DESC);

CREATE TABLE source_collection_sync_rules (
    collection_id      INTEGER PRIMARY KEY REFERENCES source_collections(id) ON DELETE CASCADE,
    default_included   INTEGER NOT NULL DEFAULT 0 CHECK (default_included IN (0, 1)),
    updated_at         INTEGER NOT NULL
);

CREATE TABLE source_track_sync_overrides (
    collection_id    INTEGER NOT NULL REFERENCES source_collections(id) ON DELETE CASCADE,
    source_track_id  INTEGER NOT NULL REFERENCES source_tracks(id),
    included         INTEGER NOT NULL CHECK (included IN (0, 1)),
    updated_at       INTEGER NOT NULL,
    PRIMARY KEY(collection_id, source_track_id)
);

CREATE INDEX idx_source_track_sync_overrides_track
ON source_track_sync_overrides(source_track_id);
