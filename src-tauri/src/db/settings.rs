use rusqlite::{Connection, params};

use crate::domain::AppSettings;

use super::now_ms;

pub(super) fn ensure_default(conn: &Connection) -> Result<(), rusqlite::Error> {
    let now = now_ms();
    conn.execute(
        "INSERT OR IGNORE INTO app_settings (
            id,
            keep_removed_managed_files,
            sync_on_startup,
            acquisition_enabled,
            created_at,
            updated_at
        ) VALUES (1, 1, 0, 0, ?1, ?1)",
        [now],
    )?;

    Ok(())
}

pub(super) fn get(conn: &Connection) -> Result<AppSettings, rusqlite::Error> {
    conn.query_row(
        "SELECT
            library_root,
            keep_removed_managed_files,
            sync_on_startup,
            sync_interval_minutes,
            acquisition_enabled
         FROM app_settings
         WHERE id = 1",
        [],
        |row| {
            Ok(AppSettings {
                library_root: row.get(0)?,
                keep_removed_managed_files: row.get::<_, i64>(1)? != 0,
                sync_on_startup: row.get::<_, i64>(2)? != 0,
                sync_interval_minutes: row.get(3)?,
                acquisition_enabled: row.get::<_, i64>(4)? != 0,
            })
        },
    )
}

pub(super) fn update(
    conn: &Connection,
    settings: &AppSettings,
) -> Result<AppSettings, rusqlite::Error> {
    conn.execute(
        "UPDATE app_settings
         SET library_root = ?1,
             keep_removed_managed_files = ?2,
             sync_on_startup = ?3,
             sync_interval_minutes = ?4,
             acquisition_enabled = ?5,
             updated_at = ?6
         WHERE id = 1",
        params![
            settings.library_root,
            bool_to_sql(settings.keep_removed_managed_files),
            bool_to_sql(settings.sync_on_startup),
            settings.sync_interval_minutes,
            bool_to_sql(settings.acquisition_enabled),
            now_ms(),
        ],
    )?;

    get(conn)
}

fn bool_to_sql(value: bool) -> i64 {
    if value { 1 } else { 0 }
}
