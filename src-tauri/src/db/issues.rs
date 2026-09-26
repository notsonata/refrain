use std::path::Path;

use rusqlite::Connection;

use crate::domain::{IssueKind, IssueRow};

use super::{Database, DatabaseError};

impl Database {
    pub(crate) fn non_match_issue_rows(&self) -> Result<Vec<IssueRow>, DatabaseError> {
        self.with_connection(|connection| {
            let mut issues = Vec::new();
            issues.extend(tracked_missing_local_issues(connection)?);
            issues.extend(local_only_track_issues(connection)?);
            issues.extend(inaccessible_collection_issues(connection)?);
            issues.extend(invalid_local_file_issues(connection)?);
            issues.extend(acquisition_failure_issues(connection)?);
            Ok(issues)
        })
    }
}

fn tracked_missing_local_issues(connection: &Connection) -> Result<Vec<IssueRow>, rusqlite::Error> {
    let mut statement = connection.prepare(
        "SELECT
            source.id,
            source.title,
            source.artists_json,
            (
                SELECT link.library_track_id
                FROM track_links AS link
                WHERE link.source_track_id = source.id
                ORDER BY link.library_track_id
                LIMIT 1
            ),
            (
                SELECT file.path
                FROM track_links AS link
                INNER JOIN local_files AS file
                  ON file.library_track_id = link.library_track_id
                WHERE link.source_track_id = source.id
                  AND file.state = 'missing'
                ORDER BY file.is_preferred DESC, file.id
                LIMIT 1
            )
         FROM source_tracks AS source
         WHERE source.provider = 'spotify'
           AND EXISTS (
             SELECT 1
             FROM collection_entries AS entry
             INNER JOIN source_collections AS collection
               ON collection.id = entry.collection_id
             LEFT JOIN source_collection_sync_rules AS rule
               ON rule.collection_id = collection.id
             LEFT JOIN source_track_sync_overrides AS override
               ON override.collection_id = collection.id
              AND override.source_track_id = entry.source_track_id
             WHERE entry.source_track_id = source.id
               AND entry.item_type = 'track'
               AND collection.is_accessible = 1
               AND COALESCE(override.included, rule.default_included, 0) = 1
           )
           AND NOT EXISTS (
             SELECT 1
             FROM track_links AS link
             INNER JOIN local_files AS file
               ON file.library_track_id = link.library_track_id
             WHERE link.source_track_id = source.id
               AND file.state = 'present'
           )
         ORDER BY source.normalized_artists, source.normalized_title, source.id",
    )?;
    statement
        .query_map([], |row| {
            let source_track_id = row.get::<_, i64>(0)?;
            let artists_json = row.get::<_, String>(2)?;
            let artists = serde_json::from_str::<Vec<String>>(&artists_json).unwrap_or_default();
            Ok(IssueRow {
                id: format!("tracked-missing:{source_track_id}"),
                kind: IssueKind::MissingLocalFile,
                title: row.get(1)?,
                subtitle: artists_subtitle(&artists),
                detail: Some(
                    "This Spotify track is selected for tracking but has no present local file."
                        .into(),
                ),
                source_track_id: Some(source_track_id),
                library_track_id: row.get(3)?,
                local_file_id: None,
                collection_id: None,
                candidate_count: None,
                confidence: None,
                path: row.get(4)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()
}

fn local_only_track_issues(connection: &Connection) -> Result<Vec<IssueRow>, rusqlite::Error> {
    let mut statement = connection.prepare(
        "SELECT
            track.id,
            track.title,
            track.artists_json,
            (
                SELECT file.path
                FROM local_files AS file
                WHERE file.library_track_id = track.id
                  AND file.state = 'present'
                ORDER BY file.is_preferred DESC, file.id
                LIMIT 1
            )
         FROM library_tracks AS track
         WHERE EXISTS (
             SELECT 1
             FROM local_files AS file
             WHERE file.library_track_id = track.id
               AND file.state = 'present'
           )
           AND NOT EXISTS (
             SELECT 1
             FROM track_links AS link
             INNER JOIN collection_entries AS entry
               ON entry.source_track_id = link.source_track_id
             INNER JOIN source_collections AS collection
               ON collection.id = entry.collection_id
             INNER JOIN source_accounts AS account
               ON account.id = collection.source_account_id
             WHERE link.library_track_id = track.id
               AND account.provider = 'spotify'
               AND collection.is_accessible = 1
               AND entry.item_type = 'track'
           )
           AND EXISTS (
             SELECT 1
             FROM source_accounts AS account
             WHERE account.provider = 'spotify'
           )
         ORDER BY track.normalized_artists, track.normalized_title, track.id",
    )?;
    statement
        .query_map([], |row| {
            let library_track_id = row.get::<_, i64>(0)?;
            let artists_json = row.get::<_, String>(2)?;
            let artists = serde_json::from_str::<Vec<String>>(&artists_json).unwrap_or_default();
            Ok(IssueRow {
                id: format!("local-only:{library_track_id}"),
                kind: IssueKind::LocalOnlyTrack,
                title: row.get(1)?,
                subtitle: artists_subtitle(&artists),
                detail: Some(
                    "This track exists locally but is not present in the imported Spotify source state."
                        .into(),
                ),
                source_track_id: None,
                library_track_id: Some(library_track_id),
                local_file_id: None,
                collection_id: None,
                candidate_count: None,
                confidence: None,
                path: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()
}

fn inaccessible_collection_issues(
    connection: &Connection,
) -> Result<Vec<IssueRow>, rusqlite::Error> {
    let mut statement = connection.prepare(
        "SELECT id, kind, name, access_issue
         FROM source_collections
         WHERE is_accessible = 0
         ORDER BY lower(name), id",
    )?;
    statement
        .query_map([], |row| {
            let collection_id = row.get::<_, i64>(0)?;
            let kind = row.get::<_, String>(1)?;
            Ok(IssueRow {
                id: format!("collection:{collection_id}"),
                kind: IssueKind::InaccessibleCollection,
                title: row.get(2)?,
                subtitle: Some(format!("{} is inaccessible", collection_kind_label(&kind))),
                detail: row.get(3)?,
                source_track_id: None,
                library_track_id: None,
                local_file_id: None,
                collection_id: Some(collection_id),
                candidate_count: None,
                confidence: None,
                path: None,
            })
        })?
        .collect::<Result<Vec<_>, _>>()
}

fn invalid_local_file_issues(connection: &Connection) -> Result<Vec<IssueRow>, rusqlite::Error> {
    let mut statement = connection.prepare(
        "SELECT
            file.id,
            file.library_track_id,
            file.path,
            file.scan_error,
            track.title,
            track.artists_json
         FROM local_files AS file
         LEFT JOIN library_tracks AS track
           ON track.id = file.library_track_id
         WHERE file.state = 'invalid'
         ORDER BY lower(file.path), file.id",
    )?;
    statement
        .query_map([], |row| {
            let local_file_id = row.get::<_, i64>(0)?;
            let path = row.get::<_, String>(2)?;
            let track_title = row.get::<_, Option<String>>(4)?;
            let artists = row
                .get::<_, Option<String>>(5)?
                .and_then(|value| serde_json::from_str::<Vec<String>>(&value).ok())
                .unwrap_or_default();
            Ok(IssueRow {
                id: format!("invalid-file:{local_file_id}"),
                kind: IssueKind::InvalidLocalFile,
                title: track_title.unwrap_or_else(|| file_name(&path)),
                subtitle: artists_subtitle(&artists).or_else(|| Some("Invalid local file".into())),
                detail: row
                    .get::<_, Option<String>>(3)?
                    .or_else(|| Some("Refrain could not read this file as valid audio.".into())),
                source_track_id: None,
                library_track_id: row.get(1)?,
                local_file_id: Some(local_file_id),
                collection_id: None,
                candidate_count: None,
                confidence: None,
                path: Some(path),
            })
        })?
        .collect::<Result<Vec<_>, _>>()
}

fn acquisition_failure_issues(connection: &Connection) -> Result<Vec<IssueRow>, rusqlite::Error> {
    let mut statement = connection.prepare(
        "SELECT
            job.id,
            job.library_track_id,
            track.title,
            track.artists_json,
            job.provider,
            job.error_code,
            job.error_message,
            job.staging_path
         FROM acquisition_jobs AS job
         JOIN library_tracks AS track ON track.id = job.library_track_id
         WHERE job.status = 'failed'
         ORDER BY job.updated_at DESC, job.id DESC",
    )?;
    statement
        .query_map([], |row| {
            let job_id = row.get::<_, i64>(0)?;
            let artists_json = row.get::<_, String>(3)?;
            let artists = serde_json::from_str::<Vec<String>>(&artists_json).unwrap_or_default();
            let provider = row.get::<_, String>(4)?;
            let error_code = row.get::<_, Option<String>>(5)?;
            let error_message = row.get::<_, Option<String>>(6)?;
            let detail = match (error_code, error_message) {
                (Some(code), Some(message)) => Some(format!("{message} ({code})")),
                (None, Some(message)) => Some(message),
                (Some(code), None) => Some(format!("Acquisition failed with {code}.")),
                (None, None) => {
                    Some("The acquisition provider could not obtain this track.".into())
                }
            };
            Ok(IssueRow {
                id: format!("acquisition:{job_id}"),
                kind: IssueKind::AcquisitionFailed,
                title: row.get(2)?,
                subtitle: artists_subtitle(&artists)
                    .or_else(|| Some(format!("{provider} acquisition failed"))),
                detail,
                source_track_id: None,
                library_track_id: Some(row.get(1)?),
                local_file_id: None,
                collection_id: None,
                candidate_count: None,
                confidence: None,
                path: row.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()
}

fn artists_subtitle(artists: &[String]) -> Option<String> {
    (!artists.is_empty()).then(|| artists.join(", "))
}

fn file_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
        .to_owned()
}

fn collection_kind_label(kind: &str) -> &'static str {
    match kind {
        "playlist" => "Spotify playlist",
        "saved_album" => "Saved album",
        "liked_songs" => "Liked Songs collection",
        _ => "Spotify collection",
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use rusqlite::params;

    use super::*;

    static NEXT_DATABASE_ID: AtomicU64 = AtomicU64::new(0);

    struct TestDatabasePath(PathBuf);

    impl TestDatabasePath {
        fn new(name: &str) -> Self {
            let id = NEXT_DATABASE_ID.fetch_add(1, Ordering::Relaxed);
            Self(std::env::temp_dir().join(format!(
                "refrain-issues-{name}-{}-{id}.sqlite3",
                std::process::id()
            )))
        }
    }

    impl Drop for TestDatabasePath {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
            let _ = fs::remove_file(self.0.with_extension("sqlite3-wal"));
            let _ = fs::remove_file(self.0.with_extension("sqlite3-shm"));
        }
    }

    fn seed_ambiguous_match(database: &Database) -> (i64, i64, i64) {
        let connection = database.lock_connection().unwrap();
        let now = super::super::now_ms();
        connection
            .execute(
                "INSERT INTO source_accounts (
                    provider, provider_account_id, display_name, client_id, created_at, updated_at
                 ) VALUES ('spotify', 'listener', 'Listener', 'client', ?1, ?1)",
                [now],
            )
            .unwrap();
        let account_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO source_collections (
                    source_account_id, provider_collection_id, kind, name,
                    is_accessible, created_at, updated_at
                 ) VALUES (?1, 'playlist', 'playlist', 'Playlist', 1, ?2, ?2)",
                params![account_id, now],
            )
            .unwrap();
        let collection_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO source_tracks (
                    provider, provider_track_id, title, normalized_title, artists_json,
                    normalized_artists, album, normalized_album, duration_ms,
                    created_at, updated_at
                 ) VALUES (
                    'spotify', 'source', 'Song', 'song', '[\"Artist\"]', 'artist',
                    'Album', 'album', 180000, ?1, ?1
                 )",
                [now],
            )
            .unwrap();
        let source_track_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO collection_entries (
                    collection_id, position, source_track_id, item_type
                 ) VALUES (?1, 0, ?2, 'track')",
                params![collection_id, source_track_id],
            )
            .unwrap();

        let mut library_ids = Vec::new();
        for title in ["Song", "Song"] {
            connection
                .execute(
                    "INSERT INTO library_tracks (
                        title, normalized_title, artists_json, normalized_artists,
                        album, normalized_album, duration_ms, created_at, updated_at
                     ) VALUES (?1, 'song', '[\"Artist\"]', 'artist', 'Album', 'album', 180000, ?2, ?2)",
                    params![title, now],
                )
                .unwrap();
            library_ids.push(connection.last_insert_rowid());
        }
        (source_track_id, library_ids[0], library_ids[1])
    }

    #[test]
    fn ambiguous_match_issue_tracks_manual_decisions_without_issue_deletes() {
        let path = TestDatabasePath::new("match-review");
        let database = Database::open(path.0.clone()).unwrap();
        let (source_track_id, first_library_track_id, _second_library_track_id) =
            seed_ambiguous_match(&database);

        let initial = crate::issues::list_issues(&database, 0, 100).unwrap();
        assert_eq!(initial.counts.match_review, 1);
        let review = crate::issues::get_match_review(&database, source_track_id)
            .unwrap()
            .unwrap();
        assert_eq!(review.candidates.len(), 2);
        assert_eq!(review.outcome, crate::domain::MatchOutcome::Review);

        database
            .confirm_match(source_track_id, first_library_track_id)
            .unwrap();
        let confirmed = crate::issues::list_issues(&database, 0, 100).unwrap();
        assert_eq!(confirmed.counts.match_review, 0);
        assert_eq!(confirmed.counts.missing_local_file, 1);

        database
            .clear_match_decision(source_track_id, None)
            .unwrap();
        let cleared = crate::issues::list_issues(&database, 0, 100).unwrap();
        assert_eq!(cleared.counts.match_review, 1);
        assert_eq!(cleared.counts.missing_local_file, 0);
    }

    #[test]
    fn file_and_collection_issues_clear_when_durable_state_is_repaired() {
        let path = TestDatabasePath::new("durable-state");
        let database = Database::open(path.0.clone()).unwrap();
        let connection = database.lock_connection().unwrap();
        let now = super::super::now_ms();
        connection
            .execute(
                "INSERT INTO source_accounts (
                    provider, provider_account_id, display_name, client_id, created_at, updated_at
                 ) VALUES ('spotify', 'listener', 'Listener', 'client', ?1, ?1)",
                [now],
            )
            .unwrap();
        let account_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO source_collections (
                    source_account_id, provider_collection_id, kind, name,
                    is_accessible, access_issue, created_at, updated_at
                 ) VALUES (?1, 'blocked', 'playlist', 'Blocked Playlist', 0, 'Spotify denied access', ?2, ?2)",
                params![account_id, now],
            )
            .unwrap();
        let collection_id = connection.last_insert_rowid();
        connection
            .execute(
                "INSERT INTO local_files (
                    path, ownership, state, file_size, modified_at, scan_error, created_at, updated_at
                 ) VALUES
                    ('/music/missing.flac', 'external', 'missing', 100, 1, NULL, ?1, ?1),
                    ('/music/invalid.flac', 'external', 'invalid', 100, 1, 'Unreadable tags', ?1, ?1)",
                [now],
            )
            .unwrap();
        drop(connection);

        let initial = crate::issues::list_issues(&database, 0, 100).unwrap();
        assert_eq!(initial.counts.missing_local_file, 1);
        assert_eq!(initial.counts.invalid_local_file, 1);
        assert_eq!(initial.counts.inaccessible_collection, 1);

        let connection = database.lock_connection().unwrap();
        connection
            .execute(
                "UPDATE local_files SET state = 'present', scan_error = NULL",
                [],
            )
            .unwrap();
        connection
            .execute(
                "UPDATE source_collections
                 SET is_accessible = 1, access_issue = NULL
                 WHERE id = ?1",
                [collection_id],
            )
            .unwrap();
        drop(connection);

        let repaired = crate::issues::list_issues(&database, 0, 100).unwrap();
        assert_eq!(repaired.counts.missing_local_file, 0);
        assert_eq!(repaired.counts.invalid_local_file, 0);
        assert_eq!(repaired.counts.inaccessible_collection, 0);
        assert_eq!(repaired.total, 0);
    }
}
