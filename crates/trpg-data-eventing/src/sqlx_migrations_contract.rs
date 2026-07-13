crate::define_data_event_module!(
    SqlxMigrationsContractCommand,
    SqlxMigrationsContractOperation,
    append_sqlx_migrations_contract_event,
    "sqlx_migrations_contract",
    "SqlxMigrationsContractRecorded",
    "data_eventing.sqlx_migrations_contract.event_schema",
    crate::DataEventOperation::MigrationRecord,
    ["migration_ledger", "schema_registry"]
);

crate::define_data_event_artifacts!(
    SqlxMigrationsContractEvent,
    SqlxMigrationsContractError,
    EVENT_TYPE,
    EVENT_SCHEMA_NAME
);

pub const REQUIRED_EVENT_STORE_COLUMNS: &[&str] = &[
    "idempotency_key",
    "expected_version",
    "authority_contract_version",
    "visibility_label",
    "fact_provenance_kind",
    "correlation_id",
    "causation_id",
];

pub fn required_event_store_columns() -> &'static [&'static str] {
    REQUIRED_EVENT_STORE_COLUMNS
}
