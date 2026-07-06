crate::define_data_event_module!(
    PersistencePostgresqlImplCommand,
    PersistencePostgresqlImplOperation,
    append_persistence_postgresql_impl_event,
    "CODEX-0638-06-DATA-EVENTING-0f91c8671e",
    "persistence_postgresql_impl",
    "PersistencePostgresqlImplRecorded",
    "data_eventing.persistence_postgresql_impl.event_schema",
    crate::DataEventOperation::EventStoreAppend,
    ["event_store", "event_outbox", "projection_checkpoint"]
);

crate::define_data_event_artifacts!(
    PersistencePostgresqlImplService,
    PersistencePostgresqlImplRepository,
    PersistencePostgresqlImplEvent,
    PersistencePostgresqlImplError,
    EVENT_TYPE,
    EVENT_SCHEMA_NAME
);

pub const TRANSACTIONAL_TABLES: &[&str] = &[
    crate::EVENT_STORE_TABLE,
    crate::OUTBOX_TABLE,
    "projection_checkpoint",
];
