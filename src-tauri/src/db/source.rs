use rusqlite::{Connection, params};

use crate::domain::{CollectionEntry, SourceAccount, SourceCollection, SourceTrack};

use super::now_ms;

pub(super) fn upsert_account(
    conn: &Connection,
    account: &SourceAccount,
) -> Result<i64, rusqlite::Error> {
    let now = now_ms();
    conn.execute(
        "INSERT INTO source_accounts (
            provider,
            provider_account_id,
            display_name,
            client_id,
            created_at,
            updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?5)
        ON CONFLICT(provider, provider_account_id) DO UPDATE SET
            display_name = excluded.display_name,
            client_id = excluded.client_id,
            updated_at = excluded.updated_at",
        params![
            account.provider,
            account.provider_account_id,
            account.display_name,
            account.client_id,
            now,
        ],
    )?;

    conn.query_row(
        "SELECT id FROM source_accounts WHERE provider = ?1 AND provider_account_id = ?2",
        params![account.provider, account.provider_account_id],
        |row| row.get(0),
    )
}

pub(super) fn mark_account_synced(
    conn: &Connection,
    account_id: i64,
    synced_at: i64,
) -> Result<(), rusqlite::Error> {
    conn.execute(
        "UPDATE source_accounts
         SET last_source_sync_at = ?1, updated_at = ?1
         WHERE id = ?2",
        params![synced_at, account_id],
    )?;

    Ok(())
}

pub(super) fn upsert_collection(
    conn: &Connection,
    collection: &SourceCollection,
) -> Result<i64, rusqlite::Error> {
    let now = now_ms();
    conn.execute(
        "INSERT INTO source_collections (
            source_account_id,
            provider_collection_id,
            kind,
            name,
            snapshot_id,
            owner_provider_id,
            is_accessible,
            access_issue,
            created_at,
            updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)
        ON CONFLICT(source_account_id, provider_collection_id) DO UPDATE SET
            kind = excluded.kind,
            name = excluded.name,
            snapshot_id = excluded.snapshot_id,
            owner_provider_id = excluded.owner_provider_id,
            is_accessible = excluded.is_accessible,
            access_issue = excluded.access_issue,
            updated_at = excluded.updated_at",
        params![
            collection.source_account_id,
            collection.provider_collection_id,
            collection.kind,
            collection.name,
            collection.snapshot_id,
            collection.owner_provider_id,
            bool_to_sql(collection.is_accessible),
            collection.access_issue,
            now,
        ],
    )?;

    conn.query_row(
        "SELECT id FROM source_collections
         WHERE source_account_id = ?1 AND provider_collection_id = ?2",
        params![
            collection.source_account_id,
            collection.provider_collection_id
        ],
        |row| row.get(0),
    )
}

pub(super) fn upsert_track(conn: &Connection, track: &SourceTrack) -> Result<i64, rusqlite::Error> {
    let now = now_ms();
    conn.execute(
        "INSERT INTO source_tracks (
            provider,
            provider_track_id,
            uri,
            isrc,
            title,
            normalized_title,
            artists_json,
            normalized_artists,
            album,
            normalized_album,
            duration_ms,
            disc_number,
            track_number,
            release_year,
            explicit,
            version_kind,
            version_detail,
            image_url,
            external_url,
            created_at,
            updated_at
        ) VALUES (
            ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
            ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?20
        )
        ON CONFLICT(provider, provider_track_id) DO UPDATE SET
            uri = excluded.uri,
            isrc = excluded.isrc,
            title = excluded.title,
            normalized_title = excluded.normalized_title,
            artists_json = excluded.artists_json,
            normalized_artists = excluded.normalized_artists,
            album = excluded.album,
            normalized_album = excluded.normalized_album,
            duration_ms = excluded.duration_ms,
            disc_number = excluded.disc_number,
            track_number = excluded.track_number,
            release_year = excluded.release_year,
            explicit = excluded.explicit,
            version_kind = excluded.version_kind,
            version_detail = excluded.version_detail,
            image_url = excluded.image_url,
            external_url = excluded.external_url,
            updated_at = excluded.updated_at",
        params![
            track.provider,
            track.provider_track_id,
            track.uri,
            track.isrc,
            track.title,
            track.normalized_title,
            track.artists_json,
            track.normalized_artists,
            track.album,
            track.normalized_album,
            track.duration_ms,
            track.disc_number,
            track.track_number,
            track.release_year,
            track.explicit.map(bool_to_sql),
            track.version_kind,
            track.version_detail,
            track.image_url,
            track.external_url,
            now,
        ],
    )?;

    conn.query_row(
        "SELECT id FROM source_tracks WHERE provider = ?1 AND provider_track_id = ?2",
        params![track.provider, track.provider_track_id],
        |row| row.get(0),
    )
}

pub(super) fn replace_collection_entries(
    conn: &mut Connection,
    collection_id: i64,
    entries: &[CollectionEntry],
) -> Result<(), rusqlite::Error> {
    let tx = conn.transaction()?;
    tx.execute(
        "DELETE FROM collection_entries WHERE collection_id = ?1",
        [collection_id],
    )?;

    {
        let mut statement = tx.prepare(
            "INSERT INTO collection_entries (
                collection_id,
                position,
                source_track_id,
                provider_item_uri,
                item_type,
                added_at,
                unavailable_reason
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        )?;

        for entry in entries {
            statement.execute(params![
                collection_id,
                entry.position,
                entry.source_track_id,
                entry.provider_item_uri,
                entry.item_type,
                entry.added_at,
                entry.unavailable_reason,
            ])?;
        }
    }

    tx.commit()
}

fn bool_to_sql(value: bool) -> i64 {
    if value { 1 } else { 0 }
}
