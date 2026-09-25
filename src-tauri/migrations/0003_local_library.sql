CREATE TABLE library_tracks (
    id                         INTEGER PRIMARY KEY,
    canonical_source_track_id  INTEGER NULL REFERENCES source_tracks(id),
    title                      TEXT NOT NULL,
    normalized_title           TEXT NOT NULL,
    artists_json               TEXT NOT NULL,
    normalized_artists         TEXT NOT NULL,
    album                      TEXT NULL,
    normalized_album           TEXT NULL,
    isrc                       TEXT NULL,
    duration_ms                INTEGER NULL,
    disc_number                INTEGER NULL,
    track_number               INTEGER NULL,
    release_year               INTEGER NULL,
    explicit                   INTEGER NULL CHECK (explicit IS NULL OR explicit IN (0, 1)),
    version_kind               TEXT NULL,
    version_detail             TEXT NULL,
    created_at                 INTEGER NOT NULL,
    updated_at                 INTEGER NOT NULL
);

CREATE TABLE local_files (
    id                INTEGER PRIMARY KEY,
    library_track_id  INTEGER NULL REFERENCES library_tracks(id),
    path              TEXT NOT NULL,
    ownership         TEXT NOT NULL CHECK (ownership IN ('managed', 'external')),
    is_preferred      INTEGER NOT NULL DEFAULT 0 CHECK (is_preferred IN (0, 1)),
    state             TEXT NOT NULL CHECK (state IN ('present', 'missing', 'invalid')),
    format            TEXT NULL,
    file_size         INTEGER NOT NULL,
    modified_at       INTEGER NOT NULL,
    duration_ms       INTEGER NULL,
    bitrate           INTEGER NULL,
    sample_rate       INTEGER NULL,
    channels          INTEGER NULL,
    content_hash      TEXT NULL,
    tag_title         TEXT NULL,
    tag_artists_json  TEXT NULL,
    tag_album         TEXT NULL,
    tag_isrc          TEXT NULL,
    scan_error        TEXT NULL,
    created_at        INTEGER NOT NULL,
    updated_at        INTEGER NOT NULL,
    UNIQUE(path)
);

CREATE INDEX idx_library_tracks_isrc ON library_tracks(isrc);
CREATE INDEX idx_library_tracks_normalized_title ON library_tracks(normalized_title);
CREATE INDEX idx_library_tracks_normalized_artists ON library_tracks(normalized_artists);
CREATE INDEX idx_local_files_library_track_id ON local_files(library_track_id);
CREATE INDEX idx_local_files_content_hash ON local_files(content_hash);
CREATE INDEX idx_local_files_state ON local_files(state);
