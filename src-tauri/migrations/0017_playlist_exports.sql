CREATE TABLE playlist_exports (
    id              INTEGER PRIMARY KEY,
    collection_id   INTEGER NOT NULL REFERENCES source_collections(id),
    mode            TEXT NOT NULL CHECK (mode IN ('m3u8', 'bundle')),
    destination     TEXT NOT NULL,
    status          TEXT NOT NULL CHECK (status IN ('running', 'succeeded', 'failed')),
    created_at      INTEGER NOT NULL,
    finished_at     INTEGER NULL,
    error_message   TEXT NULL
);

CREATE TABLE playlist_export_entries (
    export_id               INTEGER NOT NULL REFERENCES playlist_exports(id) ON DELETE CASCADE,
    position                INTEGER NOT NULL,
    source_track_id         INTEGER NOT NULL REFERENCES source_tracks(id),
    library_track_id        INTEGER NOT NULL REFERENCES library_tracks(id),
    local_file_id           INTEGER NOT NULL REFERENCES local_files(id),
    exported_relative_path  TEXT NOT NULL,
    PRIMARY KEY(export_id, position)
);

CREATE INDEX idx_playlist_exports_collection_created
    ON playlist_exports(collection_id, created_at DESC, id DESC);

CREATE INDEX idx_playlist_export_entries_local_file
    ON playlist_export_entries(local_file_id);
