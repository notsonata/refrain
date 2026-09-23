use rusqlite_migration::{M, Migrations};

const MIGRATION_ARRAY: &[M] = &[M::up(include_str!(
    "../../migrations/0001_v0_1_source.sql"
))];

pub static MIGRATIONS: Migrations = Migrations::from_slice(MIGRATION_ARRAY);
