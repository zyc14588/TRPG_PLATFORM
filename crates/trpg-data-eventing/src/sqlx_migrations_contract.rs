crate::define_data_event_module!(
    SqlxMigrationsContractCommand,
    SqlxMigrationsContractOperation,
    append_sqlx_migrations_contract_event,
    "CODEX-0625-06-DATA-EVENTING-181b11b4cd",
    "sqlx_migrations_contract",
    "SqlxMigrationsContractRecorded",
    "data_eventing.sqlx_migrations_contract.event_schema",
    crate::DataEventOperation::MigrationRecord,
    ["migration_ledger", "schema_registry"]
);

crate::define_data_event_artifacts!(
    SqlxMigrationsContractService,
    SqlxMigrationsContractRepository,
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
