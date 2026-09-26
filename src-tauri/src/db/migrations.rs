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
];

pub static MIGRATIONS: Migrations = Migrations::from_slice(MIGRATION_ARRAY);
