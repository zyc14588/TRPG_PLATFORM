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
    SqlxMigrationsContractService,
    SqlxMigrationsContractRepository,
    SqlxMigrationsContractEvent,
    SqlxMigrationsContractError,
    EVENT_TYPE,
    EVENT_SCHEMA_NAME
);

pub const REQUIRED_EVENT_STORE_COLUMNS: &[&str] = &[
    "campaign_id",
    "stream_id",
    "stream_version",
    "event_schema_version",
    "idempotency_key",
    "idempotency_operation",
    "request_hash",
    "request_hash_source",
    "integrity_status",
    "payload_integrity_source",
    "expected_version",
    "authority_mode",
    "authority_contract_version",
    "visibility_label",
    "fact_provenance_kind",
    "correlation_id",
    "causation_id",
    "payload_json",
    "recorded_at",
];

pub const FROZEN_EVENT_STORE_MIGRATION_VERSION: i64 = 20_260_705_000_100;
pub const FROZEN_EVENT_STORE_MIGRATION_SHA384: &str =
    "40539cf7e8f2fd0a87481a7c41dc1d14b24083ceaee3dbe3ab3d6f6b38e76bbfd117942b3d20b4ef547ccb40be709379";

pub fn required_event_store_columns() -> &'static [&'static str] {
    REQUIRED_EVENT_STORE_COLUMNS
}
