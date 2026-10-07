use rusqlite::{OptionalExtension, params};

use crate::domain::{
    PlaylistExport, PlaylistExportCandidateEntry, PlaylistExportPage, PlaylistExportSnapshotEntry,
    PlaylistExportSource,
};

use super::{Database, DatabaseError, now_ms};

impl Database {
    pub(crate) fn playlist_export_source(
        &self,
        collection_id: i64,
    ) -> Result<PlaylistExportSource, DatabaseError> {
        let connection = self.lock_connection()?;
        let collection = connection
            .query_row(
                "SELECT name, kind, is_accessible, image_url
                 FROM source_collections
                 WHERE id = ?1",
                [collection_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)? != 0,
                        row.get::<_, Option<String>>(3)?,
                    ))
                },
            )
            .optional()?;
        let Some((collection_name, kind, is_accessible, image_url)) = collection else {
            return Err(DatabaseError::InvalidState(format!(
                "Spotify collection {collection_id} was not found."
            )));
        };
        if kind != "playlist" {
            return Err(DatabaseError::InvalidState(
                "Only Spotify playlists can be exported.".to_owned(),
            ));
        }
        if !is_accessible {
            return Err(DatabaseError::InvalidState(
                "This Spotify playlist is currently unavailable.".to_owned(),
            ));
        }

        let mut statement = connection.prepare(
            "SELECT
                entry.position,
                track.id,
                link.library_track_id,
                local_file.id,
                COALESCE(track.title, 'Unavailable track'),
                COALESCE(track.artists_json, '[]'),
                track.duration_ms,
                local_file.path
             FROM collection_entries AS entry
             LEFT JOIN source_tracks AS track
               ON track.id = entry.source_track_id
             LEFT JOIN track_links AS link
               ON link.source_track_id = track.id
             LEFT JOIN local_files AS local_file
               ON local_file.library_track_id = link.library_track_id
              AND local_file.is_preferred = 1
              AND local_file.state = 'present'
             WHERE entry.collection_id = ?1
               AND entry.item_type = 'track'
             ORDER BY entry.position",
        )?;
        let entries = statement
            .query_map([collection_id], |row| {
                let artists_json = row.get::<_, String>(5)?;
                Ok(PlaylistExportCandidateEntry {
                    position: row.get(0)?,
                    source_track_id: row.get(1)?,
                    library_track_id: row.get(2)?,
                    local_file_id: row.get(3)?,
                    title: row.get(4)?,
                    artists: serde_json::from_str::<Vec<String>>(&artists_json).unwrap_or_default(),
                    duration_ms: row.get(6)?,
                    file_path: row.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(PlaylistExportSource {
            collection_id,
            collection_name,
            image_url,
            entries,
        })
    }

    pub(crate) fn create_playlist_export_snapshot(
        &self,
        collection_id: i64,
        mode: &str,
        destination: &str,
        entries: &[PlaylistExportSnapshotEntry],
    ) -> Result<PlaylistExport, DatabaseError> {
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        let created_at = now_ms();
        transaction.execute(
            "INSERT INTO playlist_exports (
                collection_id, mode, destination, status, created_at
             ) VALUES (?1, ?2, ?3, 'running', ?4)",
            params![collection_id, mode, destination, created_at],
        )?;
        let export_id = transaction.last_insert_rowid();
        for entry in entries {
            transaction.execute(
                "INSERT INTO playlist_export_entries (
                    export_id, position, source_track_id, library_track_id,
                    local_file_id, exported_relative_path
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    export_id,
                    entry.position,
                    entry.source_track_id,
                    entry.library_track_id,
                    entry.local_file_id,
                    entry.exported_relative_path,
                ],
            )?;
        }
        transaction.commit()?;
        drop(connection);
        self.playlist_export(export_id)?.ok_or_else(|| {
            DatabaseError::InvalidState("Playlist export snapshot could not be loaded.".to_owned())
        })
    }

    pub(crate) fn finish_playlist_export(
        &self,
        export_id: i64,
        status: &str,
        error_message: Option<&str>,
    ) -> Result<PlaylistExport, DatabaseError> {
        let connection = self.lock_connection()?;
        connection.execute(
            "UPDATE playlist_exports
             SET status = ?2, finished_at = ?3, error_message = ?4
             WHERE id = ?1",
            params![export_id, status, now_ms(), error_message],
        )?;
        drop(connection);
        self.playlist_export(export_id)?.ok_or_else(|| {
            DatabaseError::InvalidState("Playlist export result could not be loaded.".to_owned())
        })
    }

    pub fn playlist_exports_page(
        &self,
        collection_id: Option<i64>,
        offset: u32,
        limit: u32,
    ) -> Result<PlaylistExportPage, DatabaseError> {
        let limit = limit.clamp(1, 100);
        let connection = self.lock_connection()?;
        let total = connection.query_row(
            "SELECT COUNT(*)
             FROM playlist_exports
             WHERE (?1 IS NULL OR collection_id = ?1)",
            [collection_id],
            |row| row.get::<_, i64>(0),
        )?;
        let mut statement = connection.prepare(
            "SELECT
                export.id,
                export.collection_id,
                collection.name,
                export.mode,
                export.destination,
                export.status,
                COUNT(entry.position),
                export.created_at,
                export.finished_at,
                export.error_message
             FROM playlist_exports AS export
             INNER JOIN source_collections AS collection
               ON collection.id = export.collection_id
             LEFT JOIN playlist_export_entries AS entry
               ON entry.export_id = export.id
             WHERE (?1 IS NULL OR export.collection_id = ?1)
             GROUP BY export.id
             ORDER BY export.created_at DESC, export.id DESC
             LIMIT ?2 OFFSET ?3",
        )?;
        let items = statement
            .query_map(
                params![collection_id, i64::from(limit), i64::from(offset)],
                export_from_row,
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(PlaylistExportPage {
            items,
            total,
            offset,
            limit,
        })
    }

    fn playlist_export(&self, export_id: i64) -> Result<Option<PlaylistExport>, DatabaseError> {
        let connection = self.lock_connection()?;
        connection
            .query_row(
                "SELECT
                    export.id,
                    export.collection_id,
                    collection.name,
                    export.mode,
                    export.destination,
                    export.status,
                    COUNT(entry.position),
                    export.created_at,
                    export.finished_at,
                    export.error_message
                 FROM playlist_exports AS export
                 INNER JOIN source_collections AS collection
                   ON collection.id = export.collection_id
                 LEFT JOIN playlist_export_entries AS entry
                   ON entry.export_id = export.id
                 WHERE export.id = ?1
                 GROUP BY export.id",
                [export_id],
                export_from_row,
            )
            .optional()
            .map_err(DatabaseError::from)
    }
}

fn export_from_row(row: &rusqlite::Row<'_>) -> Result<PlaylistExport, rusqlite::Error> {
    Ok(PlaylistExport {
        id: row.get(0)?,
        collection_id: row.get(1)?,
        collection_name: row.get(2)?,
        mode: row.get(3)?,
        destination: row.get(4)?,
        status: row.get(5)?,
        entry_count: row.get::<_, i64>(6)? as usize,
        created_at: row.get(7)?,
        finished_at: row.get(8)?,
        error_message: row.get(9)?,
    })
}
