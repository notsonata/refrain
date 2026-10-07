use rusqlite_migration::{M, Migrations};

const MIGRATION_ARRAY: &[M] = &[
    M::up(include_str!("../../migrations/0001_v0_1_source.sql")),
    M::up(include_str!("../../migrations/0002_spotify_auth.sql")),
    M::up(include_str!("../../migrations/0003_local_library.sql")),
    M::up(include_str!("../../migrations/0004_matching.sql")),
    M::up(include_str!("../../migrations/0005_reconciliation.sql")),
    M::up(include_str!("../../migrations/0006_acquisition.sql")),
    M::up(include_str!(
        "../../migrations/0007_sync_scopes_and_tracking.sql"
    )),
    M::up(include_str!("../../migrations/0008_local_artwork.sql")),
    M::up(include_str!("../../migrations/0009_album_metadata.sql")),
    M::up(include_str!("../../migrations/0010_collection_artwork.sql")),
    M::up(include_str!(
        "../../migrations/0011_source_account_profile_image.sql"
    )),
    M::up(include_str!("../../migrations/0012_local_file_year.sql")),
    M::up(include_str!(
        "../../migrations/0013_acquisition_staging.sql"
    )),
    M::up(include_str!("../../migrations/0014_local_playlists.sql")),
    M::up(include_str!(
        "../../migrations/0015_acquisition_provider.sql"
    )),
    M::up(include_str!(
        "../../migrations/0016_acquisition_provider_chain.sql"
    )),
    M::up(include_str!("../../migrations/0017_playlist_exports.sql")),
    M::up(include_str!(
        "../../migrations/0018_spotify_playlist_mirrors.sql"
    )),
    M::up(include_str!(
        "../../migrations/0019_managed_local_playlist_files.sql"
    )),
    M::up(include_str!(
        "../../migrations/0020_local_playlist_cover_sidecars.sql"
    )),
];

#[cfg(test)]
pub(super) const MIGRATION_COUNT: i64 = MIGRATION_ARRAY.len() as i64;
pub static MIGRATIONS: Migrations = Migrations::from_slice(MIGRATION_ARRAY);
