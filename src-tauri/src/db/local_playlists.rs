use rusqlite::{Connection, OptionalExtension, params};

use crate::domain::{LocalPlaylistDetail, LocalPlaylistEntry, LocalPlaylistSummary};

use super::{Database, DatabaseError, now_ms};

impl Database {
    pub fn list_local_playlists(&self) -> Result<Vec<LocalPlaylistSummary>, DatabaseError> {
        let connection = self.lock_connection()?;
        let mut statement = connection.prepare(
            "SELECT
                playlist.id,
                playlist.name,
                playlist.source_collection_id,
                collection.image_url,
                COUNT(entry.id),
                playlist.m3u_path,
                playlist.m3u_managed,
                playlist.last_synced_at,
                playlist.sync_error,
                playlist.created_at,
                playlist.updated_at
             FROM local_playlists AS playlist
             LEFT JOIN source_collections AS collection
               ON collection.id = playlist.source_collection_id
             LEFT JOIN local_playlist_entries AS entry
               ON entry.playlist_id = playlist.id
             GROUP BY playlist.id
             ORDER BY lower(playlist.name), playlist.id",
        )?;
        statement
            .query_map([], |row| {
                Ok(LocalPlaylistSummary {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    source_collection_id: row.get(2)?,
                    image_url: row.get(3)?,
                    entry_count: row.get::<_, i64>(4)? as usize,
                    m3u_path: row.get(5)?,
                    m3u_managed: row.get::<_, i64>(6)? != 0,
                    last_synced_at: row.get(7)?,
                    sync_error: row.get(8)?,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(DatabaseError::from)
    }

    pub fn get_local_playlist(
        &self,
        playlist_id: i64,
    ) -> Result<LocalPlaylistDetail, DatabaseError> {
        let connection = self.lock_connection()?;
        local_playlist_detail(&connection, playlist_id)
    }

    pub fn create_local_playlist(&self, name: &str) -> Result<LocalPlaylistDetail, DatabaseError> {
        let name = validate_playlist_name(name)?;
        let connection = self.lock_connection()?;
        let now = now_ms();
        connection.execute(
            "INSERT INTO local_playlists (name, m3u_managed, created_at, updated_at)
             VALUES (?1, 1, ?2, ?2)",
            params![name, now],
        )?;
        local_playlist_detail(&connection, connection.last_insert_rowid())
    }

    pub(crate) fn import_local_playlist_from_m3u(
        &self,
        name: &str,
        path: &str,
        library_track_ids: &[i64],
    ) -> Result<LocalPlaylistDetail, DatabaseError> {
        let name = validate_playlist_name(name)?;
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        let now = now_ms();
        let existing_id = transaction
            .query_row(
                "SELECT id
                 FROM local_playlists
                 WHERE m3u_path = ?1
                 ORDER BY id
                 LIMIT 1",
                [path],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        if let Some(id) = existing_id {
            transaction.commit()?;
            return local_playlist_detail(&connection, id);
        }
        transaction.execute(
            "INSERT INTO local_playlists (name, m3u_path, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?3)",
            params![name, path, now],
        )?;
        let playlist_id = transaction.last_insert_rowid();

        transaction.execute(
            "DELETE FROM local_playlist_entries WHERE playlist_id = ?1",
            [playlist_id],
        )?;
        for (position, library_track_id) in library_track_ids.iter().enumerate() {
            transaction.execute(
                "INSERT INTO local_playlist_entries (
                    playlist_id, position, library_track_id, created_at
                 ) VALUES (?1, ?2, ?3, ?4)",
                params![playlist_id, position as i64, library_track_id, now],
            )?;
        }
        transaction.execute(
            "UPDATE local_playlists
             SET sync_error = NULL, updated_at = ?2
             WHERE id = ?1",
            params![playlist_id, now],
        )?;
        transaction.commit()?;
        local_playlist_detail(&connection, playlist_id)
    }

    pub fn rename_local_playlist(
        &self,
        playlist_id: i64,
        name: &str,
    ) -> Result<LocalPlaylistDetail, DatabaseError> {
        let name = validate_playlist_name(name)?;
        let connection = self.lock_connection()?;
        ensure_playlist_editable(&connection, playlist_id)?;
        let changed = connection.execute(
            "UPDATE local_playlists
             SET name = ?2, updated_at = ?3
             WHERE id = ?1",
            params![playlist_id, name, now_ms()],
        )?;
        ensure_playlist_changed(changed, playlist_id)?;
        local_playlist_detail(&connection, playlist_id)
    }

    pub fn delete_local_playlist(&self, playlist_id: i64) -> Result<(), DatabaseError> {
        let connection = self.lock_connection()?;
        ensure_playlist_editable(&connection, playlist_id)?;
        let changed =
            connection.execute("DELETE FROM local_playlists WHERE id = ?1", [playlist_id])?;
        ensure_playlist_changed(changed, playlist_id)
    }

    pub fn set_local_playlist_m3u_path(
        &self,
        playlist_id: i64,
        path: Option<&str>,
    ) -> Result<LocalPlaylistDetail, DatabaseError> {
        let normalized = path.and_then(|value| {
            let trimmed = value.trim();
            (!trimmed.is_empty()).then_some(trimmed)
        });
        let connection = self.lock_connection()?;
        let changed = connection.execute(
            "UPDATE local_playlists
             SET m3u_path = ?2, m3u_managed = 0, sync_error = NULL, updated_at = ?3
             WHERE id = ?1",
            params![playlist_id, normalized, now_ms()],
        )?;
        ensure_playlist_changed(changed, playlist_id)?;
        local_playlist_detail(&connection, playlist_id)
    }

    pub(crate) fn set_local_playlist_managed_m3u_path(
        &self,
        playlist_id: i64,
        path: &str,
    ) -> Result<LocalPlaylistDetail, DatabaseError> {
        let connection = self.lock_connection()?;
        let changed = connection.execute(
            "UPDATE local_playlists
             SET m3u_path = ?2, m3u_managed = 1, sync_error = NULL, updated_at = ?3
             WHERE id = ?1",
            params![playlist_id, path, now_ms()],
        )?;
        ensure_playlist_changed(changed, playlist_id)?;
        local_playlist_detail(&connection, playlist_id)
    }

    pub fn add_tracks_to_local_playlist(
        &self,
        playlist_id: i64,
        library_track_ids: &[i64],
    ) -> Result<LocalPlaylistDetail, DatabaseError> {
        if library_track_ids.is_empty() {
            return self.get_local_playlist(playlist_id);
        }

        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        ensure_playlist_editable(&transaction, playlist_id)?;
        let start_position = transaction.query_row(
            "SELECT COALESCE(MAX(position), -1) + 1
             FROM local_playlist_entries
             WHERE playlist_id = ?1",
            [playlist_id],
            |row| row.get::<_, i64>(0),
        )?;
        let now = now_ms();

        for (offset, library_track_id) in library_track_ids.iter().enumerate() {
            let exists = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM library_tracks WHERE id = ?1)",
                [library_track_id],
                |row| row.get::<_, i64>(0),
            )? != 0;
            if !exists {
                return Err(DatabaseError::InvalidState(format!(
                    "Local track {library_track_id} no longer exists."
                )));
            }
            transaction.execute(
                "INSERT INTO local_playlist_entries (
                    playlist_id, position, library_track_id, created_at
                 ) VALUES (?1, ?2, ?3, ?4)",
                params![
                    playlist_id,
                    start_position + offset as i64,
                    library_track_id,
                    now
                ],
            )?;
        }
        transaction.execute(
            "UPDATE local_playlists
             SET sync_error = NULL, updated_at = ?2
             WHERE id = ?1",
            params![playlist_id, now],
        )?;
        transaction.commit()?;
        local_playlist_detail(&connection, playlist_id)
    }

    pub fn remove_local_playlist_entry(
        &self,
        playlist_id: i64,
        entry_id: i64,
    ) -> Result<LocalPlaylistDetail, DatabaseError> {
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        ensure_playlist_editable(&transaction, playlist_id)?;
        let changed = transaction.execute(
            "DELETE FROM local_playlist_entries
             WHERE id = ?1 AND playlist_id = ?2",
            params![entry_id, playlist_id],
        )?;
        if changed == 0 {
            return Err(DatabaseError::InvalidState(format!(
                "Playlist entry {entry_id} was not found."
            )));
        }
        reindex_playlist_entries(&transaction, playlist_id)?;
        transaction.execute(
            "UPDATE local_playlists
             SET sync_error = NULL, updated_at = ?2
             WHERE id = ?1",
            params![playlist_id, now_ms()],
        )?;
        transaction.commit()?;
        local_playlist_detail(&connection, playlist_id)
    }

    pub fn move_local_playlist_entry(
        &self,
        playlist_id: i64,
        entry_id: i64,
        new_position: usize,
    ) -> Result<LocalPlaylistDetail, DatabaseError> {
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        ensure_playlist_editable(&transaction, playlist_id)?;
        let mut statement = transaction.prepare(
            "SELECT id
             FROM local_playlist_entries
             WHERE playlist_id = ?1
             ORDER BY position, id",
        )?;
        let mut entry_ids = statement
            .query_map([playlist_id], |row| row.get::<_, i64>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);

        let current_index = entry_ids
            .iter()
            .position(|id| *id == entry_id)
            .ok_or_else(|| {
                DatabaseError::InvalidState(format!("Playlist entry {entry_id} was not found."))
            })?;
        let entry = entry_ids.remove(current_index);
        let destination = new_position.min(entry_ids.len());
        entry_ids.insert(destination, entry);
        write_playlist_positions(&transaction, playlist_id, &entry_ids)?;
        transaction.execute(
            "UPDATE local_playlists
             SET sync_error = NULL, updated_at = ?2
             WHERE id = ?1",
            params![playlist_id, now_ms()],
        )?;
        transaction.commit()?;
        local_playlist_detail(&connection, playlist_id)
    }

    pub(crate) fn record_local_playlist_sync_success(
        &self,
        playlist_id: i64,
        synced_at: i64,
    ) -> Result<LocalPlaylistDetail, DatabaseError> {
        let connection = self.lock_connection()?;
        let changed = connection.execute(
            "UPDATE local_playlists
             SET last_synced_at = ?2, sync_error = NULL
             WHERE id = ?1",
            params![playlist_id, synced_at],
        )?;
        ensure_playlist_changed(changed, playlist_id)?;
        local_playlist_detail(&connection, playlist_id)
    }

    pub(crate) fn record_local_playlist_sync_error(
        &self,
        playlist_id: i64,
        message: &str,
    ) -> Result<(), DatabaseError> {
        let connection = self.lock_connection()?;
        let changed = connection.execute(
            "UPDATE local_playlists
             SET sync_error = ?2
             WHERE id = ?1",
            params![playlist_id, message],
        )?;
        ensure_playlist_changed(changed, playlist_id)
    }

    pub(crate) fn local_playlist_cover_state(
        &self,
        playlist_id: i64,
    ) -> Result<(Option<String>, Option<String>), DatabaseError> {
        let connection = self.lock_connection()?;
        connection
            .query_row(
                "SELECT cover_source_url, cover_path
                 FROM local_playlists
                 WHERE id = ?1",
                [playlist_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or_else(|| {
                DatabaseError::InvalidState(format!("Local playlist {playlist_id} was not found."))
            })
    }

    pub(crate) fn set_local_playlist_cover_state(
        &self,
        playlist_id: i64,
        source_url: Option<&str>,
        path: Option<&str>,
    ) -> Result<(), DatabaseError> {
        let connection = self.lock_connection()?;
        let changed = connection.execute(
            "UPDATE local_playlists
             SET cover_source_url = ?2, cover_path = ?3
             WHERE id = ?1",
            params![playlist_id, source_url, path],
        )?;
        ensure_playlist_changed(changed, playlist_id)
    }

    pub(crate) fn sync_tracked_spotify_playlist_mirrors(&self) -> Result<(), DatabaseError> {
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        let now = now_ms();

        transaction.execute(
            "DELETE FROM local_playlists
             WHERE source_collection_id IS NOT NULL
               AND NOT EXISTS (
                    SELECT 1
                    FROM source_collections AS collection
                    INNER JOIN source_collection_sync_rules AS rule
                      ON rule.collection_id = collection.id
                    WHERE collection.id = local_playlists.source_collection_id
                      AND collection.kind IN ('playlist', 'liked_songs')
                      AND rule.default_included = 1
               )",
            [],
        )?;

        let tracked_collections = {
            let mut statement = transaction.prepare(
                "SELECT collection.id, collection.name, collection.is_accessible
                 FROM source_collections AS collection
                 INNER JOIN source_collection_sync_rules AS rule
                   ON rule.collection_id = collection.id
                 WHERE collection.kind IN ('playlist', 'liked_songs')
                   AND rule.default_included = 1
                 ORDER BY collection.id",
            )?;
            statement
                .query_map([], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)? != 0,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?
        };

        for (collection_id, name, is_accessible) in tracked_collections {
            transaction.execute(
                "INSERT OR IGNORE INTO local_playlists (
                    name, source_collection_id, m3u_managed, created_at, updated_at
                 ) VALUES (?1, ?2, 1, ?3, ?3)",
                params![name, collection_id, now],
            )?;
            transaction.execute(
                "UPDATE local_playlists
                 SET updated_at = CASE WHEN name <> ?1 THEN ?3 ELSE updated_at END,
                     name = ?1
                 WHERE source_collection_id = ?2",
                params![name, collection_id, now],
            )?;
            let playlist_id = transaction.query_row(
                "SELECT id FROM local_playlists WHERE source_collection_id = ?1",
                [collection_id],
                |row| row.get::<_, i64>(0),
            )?;

            if !is_accessible {
                continue;
            }

            let desired_track_ids = {
                let mut statement = transaction.prepare(
                    "SELECT link.library_track_id
                     FROM collection_entries AS entry
                     INNER JOIN source_collection_sync_rules AS rule
                       ON rule.collection_id = entry.collection_id
                     LEFT JOIN source_track_sync_overrides AS override
                       ON override.collection_id = entry.collection_id
                      AND override.source_track_id = entry.source_track_id
                     INNER JOIN track_links AS link
                       ON link.source_track_id = entry.source_track_id
                     WHERE entry.collection_id = ?1
                       AND entry.item_type = 'track'
                       AND entry.source_track_id IS NOT NULL
                       AND COALESCE(override.included, rule.default_included, 0) = 1
                     ORDER BY entry.position",
                )?;
                statement
                    .query_map([collection_id], |row| row.get::<_, i64>(0))?
                    .collect::<Result<Vec<_>, _>>()?
            };
            let existing_track_ids = {
                let mut statement = transaction.prepare(
                    "SELECT library_track_id
                     FROM local_playlist_entries
                     WHERE playlist_id = ?1
                     ORDER BY position, id",
                )?;
                statement
                    .query_map([playlist_id], |row| row.get::<_, i64>(0))?
                    .collect::<Result<Vec<_>, _>>()?
            };
            if desired_track_ids == existing_track_ids {
                continue;
            }

            transaction.execute(
                "DELETE FROM local_playlist_entries WHERE playlist_id = ?1",
                [playlist_id],
            )?;
            for (position, library_track_id) in desired_track_ids.iter().enumerate() {
                transaction.execute(
                    "INSERT INTO local_playlist_entries (
                        playlist_id, position, library_track_id, created_at
                     ) VALUES (?1, ?2, ?3, ?4)",
                    params![playlist_id, position as i64, library_track_id, now],
                )?;
            }
            transaction.execute(
                "UPDATE local_playlists
                 SET updated_at = ?2
                 WHERE id = ?1",
                params![playlist_id, now],
            )?;
        }

        transaction.commit()?;
        Ok(())
    }
}

fn validate_playlist_name(name: &str) -> Result<&str, DatabaseError> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(DatabaseError::InvalidState(
            "Playlist name cannot be empty.".to_owned(),
        ));
    }
    Ok(trimmed)
}

fn ensure_playlist_changed(changed: usize, playlist_id: i64) -> Result<(), DatabaseError> {
    if changed == 0 {
        return Err(DatabaseError::InvalidState(format!(
            "Local playlist {playlist_id} was not found."
        )));
    }
    Ok(())
}

fn ensure_playlist_editable(
    connection: &Connection,
    playlist_id: i64,
) -> Result<(), DatabaseError> {
    let source_collection_id = connection
        .query_row(
            "SELECT source_collection_id FROM local_playlists WHERE id = ?1",
            [playlist_id],
            |row| row.get::<_, Option<i64>>(0),
        )
        .optional()?;
    let Some(source_collection_id) = source_collection_id else {
        return Err(DatabaseError::InvalidState(format!(
            "Local playlist {playlist_id} was not found."
        )));
    };
    if source_collection_id.is_some() {
        return Err(DatabaseError::InvalidState(
            "Spotify-mirrored playlists are updated from Spotify and cannot be edited directly."
                .to_owned(),
        ));
    }
    Ok(())
}

fn reindex_playlist_entries(
    connection: &Connection,
    playlist_id: i64,
) -> Result<(), DatabaseError> {
    let mut statement = connection.prepare(
        "SELECT id
         FROM local_playlist_entries
         WHERE playlist_id = ?1
         ORDER BY position, id",
    )?;
    let entry_ids = statement
        .query_map([playlist_id], |row| row.get::<_, i64>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    drop(statement);
    write_playlist_positions(connection, playlist_id, &entry_ids)
}

fn write_playlist_positions(
    connection: &Connection,
    playlist_id: i64,
    entry_ids: &[i64],
) -> Result<(), DatabaseError> {
    connection.execute(
        "UPDATE local_playlist_entries
         SET position = position + 1000000
         WHERE playlist_id = ?1",
        [playlist_id],
    )?;
    for (position, entry_id) in entry_ids.iter().enumerate() {
        connection.execute(
            "UPDATE local_playlist_entries
             SET position = ?2
             WHERE id = ?1 AND playlist_id = ?3",
            params![entry_id, position as i64, playlist_id],
        )?;
    }
    Ok(())
}

fn local_playlist_detail(
    connection: &Connection,
    playlist_id: i64,
) -> Result<LocalPlaylistDetail, DatabaseError> {
    let playlist = connection
        .query_row(
            "SELECT
                playlist.id,
                playlist.name,
                playlist.source_collection_id,
                collection.image_url,
                playlist.m3u_path,
                playlist.m3u_managed,
                playlist.last_synced_at,
                playlist.sync_error,
                playlist.created_at,
                playlist.updated_at
             FROM local_playlists AS playlist
             LEFT JOIN source_collections AS collection
               ON collection.id = playlist.source_collection_id
             WHERE playlist.id = ?1",
            [playlist_id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, i64>(5)? != 0,
                    row.get::<_, Option<i64>>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, i64>(8)?,
                    row.get::<_, i64>(9)?,
                ))
            },
        )
        .optional()?;
    let Some((
        id,
        name,
        source_collection_id,
        image_url,
        m3u_path,
        m3u_managed,
        last_synced_at,
        sync_error,
        created_at,
        updated_at,
    )) = playlist
    else {
        return Err(DatabaseError::InvalidState(format!(
            "Local playlist {playlist_id} was not found."
        )));
    };

    let mut statement = connection.prepare(
        "SELECT
            entry.id,
            entry.position,
            track.id,
            track.title,
            track.artists_json,
            track.album,
            track.duration_ms,
            preferred.path,
            preferred.artwork_path
         FROM local_playlist_entries AS entry
         INNER JOIN library_tracks AS track ON track.id = entry.library_track_id
         LEFT JOIN local_files AS preferred
           ON preferred.id = (
                SELECT file.id
                FROM local_files AS file
                WHERE file.library_track_id = track.id
                  AND file.state = 'present'
                ORDER BY file.is_preferred DESC, file.id
                LIMIT 1
           )
         WHERE entry.playlist_id = ?1
         ORDER BY entry.position, entry.id",
    )?;
    let entries = statement
        .query_map([playlist_id], |row| {
            let artists_json = row.get::<_, String>(4)?;
            Ok(LocalPlaylistEntry {
                id: row.get(0)?,
                position: row.get::<_, i64>(1)? as usize,
                library_track_id: row.get(2)?,
                title: row.get(3)?,
                artists: serde_json::from_str(&artists_json).unwrap_or_default(),
                album: row.get(5)?,
                duration_ms: row.get(6)?,
                file_path: row.get(7)?,
                artwork_path: row.get(8)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(LocalPlaylistDetail {
        id,
        name,
        source_collection_id,
        image_url,
        m3u_path,
        m3u_managed,
        last_synced_at,
        sync_error,
        created_at,
        updated_at,
        entries,
    })
}
