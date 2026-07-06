crate::define_data_event_module!(
    PersistencePostgresqlCommand,
    PersistencePostgresqlOperation,
    append_persistence_postgresql_event,
    "CODEX-0601-06-DATA-EVENTING-6b440dcd4b",
    "persistence_postgresql",
    "PersistencePostgresqlRecorded",
    "data_eventing.persistence_postgresql.event_schema",
    crate::DataEventOperation::EventStoreAppend,
    ["event_store", "event_outbox", "projection_checkpoint"]
);

crate::define_data_event_artifacts!(
    PersistencePostgresqlService,
    PersistencePostgresqlRepository,
    PersistencePostgresqlEvent,
    PersistencePostgresqlError,
    EVENT_TYPE,
    EVENT_SCHEMA_NAME
);

pub const STORAGE_TABLES: &[&str] = &["event_store", "event_outbox", "projection_checkpoint"];

pub fn required_storage_tables() -> &'static [&'static str] {
    STORAGE_TABLES
}
