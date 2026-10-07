CREATE TABLE local_playlists (
    id              INTEGER PRIMARY KEY,
    name            TEXT NOT NULL,
    m3u_path        TEXT NULL,
    last_synced_at  INTEGER NULL,
    sync_error      TEXT NULL,
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL
);

CREATE TABLE local_playlist_entries (
    id                INTEGER PRIMARY KEY,
    playlist_id       INTEGER NOT NULL REFERENCES local_playlists(id) ON DELETE CASCADE,
    position          INTEGER NOT NULL CHECK (position >= 0),
    library_track_id  INTEGER NOT NULL REFERENCES library_tracks(id),
    created_at        INTEGER NOT NULL,
    UNIQUE(playlist_id, position)
);

CREATE INDEX idx_local_playlist_entries_playlist
    ON local_playlist_entries(playlist_id, position);

CREATE INDEX idx_local_playlist_entries_library_track
    ON local_playlist_entries(library_track_id);
