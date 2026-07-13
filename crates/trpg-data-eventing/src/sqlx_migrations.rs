crate::define_data_event_module!(
    SqlxMigrationsCommand,
    SqlxMigrationsOperation,
    append_sqlx_migrations_event,
    "sqlx_migrations",
    "SqlxMigrationRecorded",
    "data_eventing.sqlx_migrations.event_schema",
    crate::DataEventOperation::MigrationRecord,
    ["migration_ledger", "event_store", "event_outbox"]
);

crate::define_data_event_artifacts!(
    SqlxMigrationsEvent,
    SqlxMigrationsError,
    EVENT_TYPE,
    EVENT_SCHEMA_NAME
);

pub fn migration_statements() -> &'static [(&'static str, &'static str)] {
    crate::persistence_migrations::migration_statements()
}
