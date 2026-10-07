ALTER TABLE local_playlists
    ADD COLUMN m3u_managed INTEGER NOT NULL DEFAULT 0
        CHECK (m3u_managed IN (0, 1));

UPDATE local_playlists
SET m3u_managed = 1
WHERE m3u_path IS NULL;
