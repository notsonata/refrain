use rusqlite::{OptionalExtension, params};

use super::{Database, DatabaseError, now_ms};

impl Database {
    pub fn set_source_collection_tracking(
        &self,
        collection_id: i64,
        included: bool,
    ) -> Result<(), DatabaseError> {
        self.with_connection(|connection| {
            let exists = connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM source_collections WHERE id = ?1)",
                [collection_id],
                |row| row.get::<_, i64>(0),
            )? != 0;
            if !exists {
                return Err(rusqlite::Error::QueryReturnedNoRows);
            }
            connection.execute(
                "INSERT INTO source_collection_sync_rules (collection_id, default_included, updated_at)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(collection_id) DO UPDATE SET
                    default_included = excluded.default_included,
                    updated_at = excluded.updated_at",
                params![collection_id, i64::from(included), now_ms()],
            )?;
            Ok(())
        })
    }

    pub fn set_source_track_tracking(
        &self,
        collection_id: i64,
        source_track_id: i64,
        included: Option<bool>,
    ) -> Result<(), DatabaseError> {
        self.set_source_tracks_tracking(collection_id, &[source_track_id], included)
    }

    pub fn set_source_tracks_tracking(
        &self,
        collection_id: i64,
        source_track_ids: &[i64],
        included: Option<bool>,
    ) -> Result<(), DatabaseError> {
        if source_track_ids.is_empty() {
            return Ok(());
        }

        let mut ids = source_track_ids.to_vec();
        ids.sort_unstable();
        ids.dedup();

        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;

        for source_track_id in &ids {
            let entry_exists = transaction
                .query_row(
                    "SELECT 1
                     FROM collection_entries
                     WHERE collection_id = ?1 AND source_track_id = ?2
                     LIMIT 1",
                    params![collection_id, source_track_id],
                    |_| Ok(()),
                )
                .optional()?
                .is_some();
            if !entry_exists {
                return Err(rusqlite::Error::QueryReturnedNoRows.into());
            }
        }

        let updated_at = now_ms();
        for source_track_id in ids {
            if let Some(included) = included {
                transaction.execute(
                    "INSERT INTO source_track_sync_overrides (
                        collection_id, source_track_id, included, updated_at
                     ) VALUES (?1, ?2, ?3, ?4)
                     ON CONFLICT(collection_id, source_track_id) DO UPDATE SET
                        included = excluded.included,
                        updated_at = excluded.updated_at",
                    params![
                        collection_id,
                        source_track_id,
                        i64::from(included),
                        updated_at
                    ],
                )?;
            } else {
                transaction.execute(
                    "DELETE FROM source_track_sync_overrides
                     WHERE collection_id = ?1 AND source_track_id = ?2",
                    params![collection_id, source_track_id],
                )?;
            }
        }

        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn tracked_source_track_ids(&self) -> Result<Vec<i64>, DatabaseError> {
        self.with_connection(|connection| {
            let mut statement = connection.prepare(
                "SELECT DISTINCT entry.source_track_id
                 FROM collection_entries AS entry
                 INNER JOIN source_collections AS collection
                    ON collection.id = entry.collection_id
                 LEFT JOIN source_collection_sync_rules AS rule
                    ON rule.collection_id = collection.id
                 LEFT JOIN source_track_sync_overrides AS override
                    ON override.collection_id = collection.id
                   AND override.source_track_id = entry.source_track_id
                 WHERE collection.is_accessible = 1
                   AND entry.item_type = 'track'
                   AND entry.source_track_id IS NOT NULL
                   AND COALESCE(override.included, rule.default_included, 0) = 1
                 ORDER BY entry.source_track_id",
            )?;
            statement
                .query_map([], |row| row.get::<_, i64>(0))?
                .collect::<Result<Vec<_>, _>>()
        })
    }
}
