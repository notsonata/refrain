use rusqlite::{OptionalExtension, params};

use crate::domain::{MirrorRun, MirrorTarget};

use super::{Database, DatabaseError, now_ms};

fn run_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<MirrorRun> {
    Ok(MirrorRun {
        id: row.get(0)?,
        target_id: row.get(1)?,
        status: row.get(2)?,
        started_at: row.get(3)?,
        finished_at: row.get(4)?,
        copied: row.get(5)?,
        unchanged: row.get(6)?,
        removed: row.get(7)?,
        error: row.get(8)?,
    })
}

impl Database {
    pub(crate) fn recover_interrupted_mirror_runs(&self) -> Result<(), DatabaseError> {
        let connection = self.lock_connection()?;
        connection.execute(
            "UPDATE mirror_runs SET status = 'failed', finished_at = ?1,
             error = 'Application stopped during mirroring. Retry to resume safely.'
             WHERE status = 'running'",
            [now_ms()],
        )?;
        Ok(())
    }

    pub fn list_mirror_targets(&self) -> Result<Vec<MirrorTarget>, DatabaseError> {
        self.load_mirror_targets(false)
    }

    pub(crate) fn all_mirror_targets(&self) -> Result<Vec<MirrorTarget>, DatabaseError> {
        self.load_mirror_targets(true)
    }

    fn load_mirror_targets(
        &self,
        including_removed: bool,
    ) -> Result<Vec<MirrorTarget>, DatabaseError> {
        let connection = self.lock_connection()?;
        let mut query = connection.prepare(
            "SELECT id, path, identity, created_at, updated_at FROM mirror_targets
             WHERE active = 1 OR ?1 = 1 ORDER BY id",
        )?;
        let mut runs = connection.prepare(
            "SELECT id, target_id, status, started_at, finished_at, copied, unchanged, removed, error
             FROM mirror_runs WHERE target_id = ?1 ORDER BY id DESC LIMIT 1",
        )?;
        let targets = query
            .query_map([including_removed as i64], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        targets
            .into_iter()
            .map(|(id, path, identity, created_at, updated_at)| {
                Ok(MirrorTarget {
                    id,
                    path,
                    identity,
                    created_at,
                    updated_at,
                    last_run: runs.query_row([id], run_from_row).optional()?,
                    available: false,
                })
            })
            .collect()
    }

    pub fn save_mirror_target(
        &self,
        path: &str,
        identity: &str,
    ) -> Result<MirrorTarget, DatabaseError> {
        let connection = self.lock_connection()?;
        let now = now_ms();
        connection.execute(
            "INSERT INTO mirror_targets (path, identity, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?3)
             ON CONFLICT(path) DO UPDATE SET identity = excluded.identity, updated_at = excluded.updated_at, active = 1",
            params![path, identity, now],
        )?;
        let id: i64 = connection.query_row(
            "SELECT id FROM mirror_targets WHERE path = ?1",
            [path],
            |row| row.get(0),
        )?;
        drop(connection);
        self.list_mirror_targets()?
            .into_iter()
            .find(|target| target.id == id)
            .ok_or_else(|| DatabaseError::InvalidState("Saved mirror target disappeared.".into()))
    }

    pub(crate) fn restore_mirror_target(
        &self,
        id: i64,
        path: &str,
        identity: &str,
    ) -> Result<MirrorTarget, DatabaseError> {
        let connection = self.lock_connection()?;
        connection.execute(
            "UPDATE mirror_targets SET path = ?2, identity = ?3, updated_at = ?4, active = 1 WHERE id = ?1",
            params![id, path, identity, now_ms()],
        )?;
        drop(connection);
        self.list_mirror_targets()?
            .into_iter()
            .find(|target| target.id == id)
            .ok_or_else(|| {
                DatabaseError::InvalidState("Restored mirror target disappeared.".into())
            })
    }

    pub fn remove_mirror_target(&self, id: i64) -> Result<(), DatabaseError> {
        let connection = self.lock_connection()?;
        connection.execute("UPDATE mirror_targets SET active = 0 WHERE id = ?1", [id])?;
        Ok(())
    }

    pub fn list_mirror_runs(&self, target_id: i64) -> Result<Vec<MirrorRun>, DatabaseError> {
        let connection = self.lock_connection()?;
        let mut query = connection.prepare(
            "SELECT id, target_id, status, started_at, finished_at, copied, unchanged, removed, error
             FROM mirror_runs WHERE target_id = ?1 ORDER BY id DESC LIMIT 20",
        )?;
        query
            .query_map([target_id], run_from_row)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(DatabaseError::from)
    }

    pub(crate) fn start_mirror_run(&self, target_id: i64) -> Result<i64, DatabaseError> {
        let connection = self.lock_connection()?;
        connection.execute(
            "INSERT INTO mirror_runs (target_id, status, started_at) VALUES (?1, 'running', ?2)",
            params![target_id, now_ms()],
        )?;
        Ok(connection.last_insert_rowid())
    }

    pub(crate) fn finish_mirror_run(
        &self,
        id: i64,
        copied: i64,
        unchanged: i64,
        removed: i64,
        error: Option<&str>,
        cancelled: bool,
    ) -> Result<(), DatabaseError> {
        let connection = self.lock_connection()?;
        connection.execute(
            "UPDATE mirror_runs SET status = ?2, finished_at = ?3,
             copied = ?4, unchanged = ?5, removed = ?6, error = ?7 WHERE id = ?1",
            params![
                id,
                if cancelled {
                    "cancelled"
                } else if error.is_some() {
                    "failed"
                } else {
                    "succeeded"
                },
                now_ms(),
                copied,
                unchanged,
                removed,
                error
            ],
        )?;
        Ok(())
    }

    pub(crate) fn replace_mirror_entries(
        &self,
        target_id: i64,
        entries: &[(String, String, i64, String)],
    ) -> Result<(), DatabaseError> {
        let mut connection = self.lock_connection()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "DELETE FROM mirror_entries WHERE target_id = ?1",
            [target_id],
        )?;
        for (path, key, size, hash) in entries {
            transaction.execute(
                "INSERT INTO mirror_entries (target_id, relative_path, source_key, file_size, content_hash)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![target_id, path, key, size, hash],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::db::MIGRATIONS;
    use rusqlite::{Connection, params};

    #[test]
    fn cancelled_status_migration_preserves_existing_runs_and_foreign_keys() {
        let mut connection = Connection::open_in_memory().unwrap();
        connection
            .pragma_update(None, "foreign_keys", "ON")
            .unwrap();
        MIGRATIONS.to_version(&mut connection, 23).unwrap();
        connection
            .execute(
                "INSERT INTO mirror_targets (id, path, identity, created_at, updated_at)
                 VALUES (1, '/test/target', 'fixture', 100, 100)",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO mirror_runs
                 (id, target_id, status, started_at, finished_at, copied, unchanged, removed, error)
                 VALUES (7, 1, 'failed', 100, 120, 2, 3, 1, 'interrupted')",
                [],
            )
            .unwrap();
        MIGRATIONS.to_latest(&mut connection).unwrap();
        let foreign_keys: i64 = connection
            .pragma_query_value(None, "foreign_keys", |row| row.get(0))
            .unwrap();
        assert_eq!(foreign_keys, 1);
        let preserved: (i64, String, i64, i64, i64, String) = connection
            .query_row(
                "SELECT id, status, copied, unchanged, removed, error FROM mirror_runs WHERE id = 7",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, row.get(5)?)),
            )
            .unwrap();
        assert_eq!(
            preserved,
            (7, "failed".into(), 2, 3, 1, "interrupted".into())
        );
        let active: i64 = connection
            .query_row(
                "SELECT active FROM mirror_targets WHERE id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(active, 1);

        connection
            .execute("UPDATE mirror_targets SET active = 0 WHERE id = 1", [])
            .unwrap();
        let retained_runs: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM mirror_runs WHERE target_id = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(retained_runs, 1);
        connection
            .execute("UPDATE mirror_targets SET active = 1 WHERE id = 1", [])
            .unwrap();

        connection
            .execute(
                "INSERT INTO mirror_runs (target_id, status, started_at) VALUES (?1, 'cancelled', ?2)",
                params![1, 130],
            )
            .unwrap();
        let violations: i64 = connection
            .query_row("SELECT COUNT(*) FROM pragma_foreign_key_check", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(violations, 0);
        connection
            .execute("DELETE FROM mirror_targets WHERE id = 1", [])
            .unwrap();
        let remaining: i64 = connection
            .query_row("SELECT COUNT(*) FROM mirror_runs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(remaining, 0);
    }
}
