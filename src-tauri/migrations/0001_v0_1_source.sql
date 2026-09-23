CREATE TABLE app_settings (
    id                          INTEGER PRIMARY KEY CHECK (id = 1),
    library_root                TEXT NULL,
    keep_removed_managed_files  INTEGER NOT NULL DEFAULT 1 CHECK (keep_removed_managed_files IN (0, 1)),
    sync_on_startup             INTEGER NOT NULL DEFAULT 0 CHECK (sync_on_startup IN (0, 1)),
    sync_interval_minutes       INTEGER NULL CHECK (sync_interval_minutes IS NULL OR sync_interval_minutes > 0),
    acquisition_enabled         INTEGER NOT NULL DEFAULT 0 CHECK (acquisition_enabled IN (0, 1)),
    created_at                  INTEGER NOT NULL,
    updated_at                  INTEGER NOT NULL
);

CREATE TABLE source_accounts (
    id                   INTEGER PRIMARY KEY,
    provider             TEXT NOT NULL,
    provider_account_id  TEXT NOT NULL,
    display_name         TEXT NULL,
    client_id            TEXT NOT NULL,
    created_at           INTEGER NOT NULL,
    updated_at           INTEGER NOT NULL,
    last_source_sync_at  INTEGER NULL,
    UNIQUE(provider, provider_account_id)
);

CREATE TABLE source_collections (
    id                      INTEGER PRIMARY KEY,
    source_account_id       INTEGER NOT NULL REFERENCES source_accounts(id),
    provider_collection_id  TEXT NOT NULL,
    kind                    TEXT NOT NULL,
    name                    TEXT NOT NULL,
    snapshot_id             TEXT NULL,
    owner_provider_id       TEXT NULL,
    is_accessible           INTEGER NOT NULL DEFAULT 1 CHECK (is_accessible IN (0, 1)),
    access_issue            TEXT NULL,
    created_at              INTEGER NOT NULL,
    updated_at              INTEGER NOT NULL,
    UNIQUE(source_account_id, provider_collection_id)
);

CREATE TABLE source_tracks (
    id                    INTEGER PRIMARY KEY,
    provider              TEXT NOT NULL,
    provider_track_id     TEXT NOT NULL,
    uri                   TEXT NULL,
    isrc                  TEXT NULL,
    title                 TEXT NOT NULL,
    normalized_title      TEXT NOT NULL,
    artists_json          TEXT NOT NULL,
    normalized_artists    TEXT NOT NULL,
    album                 TEXT NULL,
    normalized_album      TEXT NULL,
    duration_ms           INTEGER NULL,
    disc_number           INTEGER NULL,
    track_number          INTEGER NULL,
    release_year          INTEGER NULL,
    explicit              INTEGER NULL CHECK (explicit IS NULL OR explicit IN (0, 1)),
    version_kind          TEXT NULL,
    version_detail        TEXT NULL,
    image_url             TEXT NULL,
    external_url          TEXT NULL,
    created_at            INTEGER NOT NULL,
    updated_at            INTEGER NOT NULL,
    UNIQUE(provider, provider_track_id)
);

CREATE TABLE collection_entries (
    id                  INTEGER PRIMARY KEY,
    collection_id       INTEGER NOT NULL REFERENCES source_collections(id) ON DELETE CASCADE,
    position            INTEGER NOT NULL,
    source_track_id     INTEGER NULL REFERENCES source_tracks(id),
    provider_item_uri   TEXT NULL,
    item_type           TEXT NOT NULL,
    added_at            INTEGER NULL,
    unavailable_reason  TEXT NULL,
    UNIQUE(collection_id, position)
);

CREATE INDEX idx_source_tracks_isrc ON source_tracks(isrc);
CREATE INDEX idx_source_tracks_normalized_title ON source_tracks(normalized_title);
CREATE INDEX idx_source_tracks_normalized_artists ON source_tracks(normalized_artists);
CREATE INDEX idx_collection_entries_source_track_id ON collection_entries(source_track_id);
