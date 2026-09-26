use rusqlite::params;

use super::{Database, DatabaseError, now_ms};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct NormalizationCandidate {
    pub library_track_id: i64,
    pub title: String,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub release_year: Option<i64>,
    pub disc_number: Option<i64>,
    pub track_number: Option<i64>,
    pub is_multi_disc: bool,
    pub local_file_id: i64,
    pub path: String,
    pub format: Option<String>,
}

impl Database {
    pub(crate) fn normalization_candidates(
        &self,
    ) -> Result<Vec<NormalizationCandidate>, DatabaseError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT
                    track.id,
                    COALESCE(source.title, track.title),
                    COALESCE(source.artists_json, track.artists_json),
                    COALESCE(source.album, track.album),
                    COALESCE(source.release_year, track.release_year),
                    COALESCE(source.disc_number, track.disc_number),
                    COALESCE(source.track_number, track.track_number),
                    CASE
                        WHEN COALESCE(source.disc_number, track.disc_number, 1) > 1 THEN 1
                        WHEN source.id IS NOT NULL AND EXISTS (
                            SELECT 1
                            FROM source_tracks AS sibling
                            WHERE sibling.id != source.id
                              AND sibling.normalized_artists = source.normalized_artists
                              AND sibling.normalized_album IS source.normalized_album
                              AND COALESCE(sibling.disc_number, 1) > 1
                              AND EXISTS (
                                  SELECT 1
                                  FROM collection_entries AS sibling_entry
                                  INNER JOIN source_collections AS sibling_collection
                                     ON sibling_collection.id = sibling_entry.collection_id
                                  WHERE sibling_entry.source_track_id = sibling.id
                                    AND sibling_entry.item_type = 'track'
                                    AND sibling_collection.is_accessible = 1
                              )
                        ) THEN 1
                        ELSE 0
                    END AS is_multi_disc,
                    file.id,
                    file.path,
                    file.format
                 FROM library_tracks AS track
                 INNER JOIN local_files AS file
                    ON file.library_track_id = track.id
                   AND file.state = 'present'
                   AND file.is_preferred = 1
                 LEFT JOIN source_tracks AS source
                    ON source.id = (
                        SELECT link.source_track_id
                        FROM track_links AS link
                        WHERE link.library_track_id = track.id
                          AND EXISTS (
                              SELECT 1
                              FROM collection_entries AS entry
                              INNER JOIN source_collections AS collection
                                 ON collection.id = entry.collection_id
                              WHERE entry.source_track_id = link.source_track_id
                                AND entry.item_type = 'track'
                                AND collection.is_accessible = 1
                          )
                        ORDER BY link.confirmed_by_user DESC, link.source_track_id
                        LIMIT 1
                    )
                 ORDER BY track.id, file.id",
            )?;

            statement
                .query_map([], |row| {
                    let artists_json = row.get::<_, String>(2)?;
                    let artists =
                        serde_json::from_str::<Vec<String>>(&artists_json).unwrap_or_default();
                    Ok(NormalizationCandidate {
                        library_track_id: row.get(0)?,
                        title: row.get(1)?,
                        artists,
                        album: row.get(3)?,
                        release_year: row.get(4)?,
                        disc_number: row.get(5)?,
                        track_number: row.get(6)?,
                        is_multi_disc: row.get::<_, i64>(7)? != 0,
                        local_file_id: row.get(8)?,
                        path: row.get(9)?,
                        format: row.get(10)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()
        })
    }

    pub(crate) fn update_local_file_path_after_normalization(
        &self,
        local_file_id: i64,
        path: &str,
    ) -> Result<(), DatabaseError> {
        self.with_connection(|connection| {
            let changed = connection.execute(
                "UPDATE local_files
                 SET path = ?1, updated_at = ?2
                 WHERE id = ?3 AND state = 'present'",
                params![path, now_ms(), local_file_id],
            )?;
            if changed != 1 {
                return Err(rusqlite::Error::QueryReturnedNoRows);
            }
            Ok(())
        })
    }
}
