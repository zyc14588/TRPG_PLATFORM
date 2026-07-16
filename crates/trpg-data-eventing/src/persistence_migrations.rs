crate::define_data_event_module!(
    PersistenceMigrationsCommand,
    PersistenceMigrationsOperation,
    append_persistence_migrations_event,
    "persistence_migrations",
    "PersistenceMigrationRecorded",
    "data_eventing.persistence_migrations.event_schema",
    crate::DataEventOperation::MigrationRecord,
    ["migration_ledger"]
);

use sqlx::migrate::Migrator;

/// The only executable source for the primary PostgreSQL schema.
///
/// `sqlx::migrate!` embeds the bytes and SHA-384 checksums resolved directly
/// from the repository `migrations/` directory.  No Rust-owned SQL copy is
/// maintained alongside it.
pub static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

/// The external audit witness has its own database and ledger, but its SQL is
/// still sourced exclusively from the `migrations/` tree.
pub static WITNESS_MIGRATOR: Migrator = sqlx::migrate!("../../migrations/witness");

pub fn migrator() -> &'static Migrator {
    &MIGRATOR
}

pub fn witness_migrator() -> &'static Migrator {
    &WITNESS_MIGRATOR
}
