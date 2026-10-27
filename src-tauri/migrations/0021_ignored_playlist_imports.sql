-- A dismissed external M3U remains on disk and must stay dismissed on later scans.
CREATE TABLE ignored_playlist_imports (
    path TEXT PRIMARY KEY,
    ignored_at INTEGER NOT NULL
);
