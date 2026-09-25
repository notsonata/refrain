CREATE TABLE track_links (
    id                 INTEGER PRIMARY KEY,
    source_track_id    INTEGER NOT NULL REFERENCES source_tracks(id) ON DELETE CASCADE,
    library_track_id   INTEGER NOT NULL REFERENCES library_tracks(id) ON DELETE CASCADE,
    method             TEXT NOT NULL CHECK (method IN ('existing', 'isrc', 'metadata', 'acquisition', 'user')),
    confidence         INTEGER NOT NULL CHECK (confidence BETWEEN 0 AND 10000),
    confirmed_by_user  INTEGER NOT NULL DEFAULT 0 CHECK (confirmed_by_user IN (0, 1)),
    created_at         INTEGER NOT NULL,
    updated_at         INTEGER NOT NULL,
    UNIQUE(source_track_id)
);

CREATE TABLE track_rejections (
    source_track_id   INTEGER NOT NULL REFERENCES source_tracks(id) ON DELETE CASCADE,
    library_track_id  INTEGER NOT NULL REFERENCES library_tracks(id) ON DELETE CASCADE,
    reason            TEXT NULL,
    created_at        INTEGER NOT NULL,
    PRIMARY KEY(source_track_id, library_track_id)
);

CREATE INDEX idx_track_links_library_track_id ON track_links(library_track_id);
