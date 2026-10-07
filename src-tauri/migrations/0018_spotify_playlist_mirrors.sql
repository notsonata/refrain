ALTER TABLE local_playlists
    ADD COLUMN source_collection_id INTEGER NULL
        REFERENCES source_collections(id) ON DELETE CASCADE;

CREATE UNIQUE INDEX idx_local_playlists_source_collection
    ON local_playlists(source_collection_id)
    WHERE source_collection_id IS NOT NULL;
