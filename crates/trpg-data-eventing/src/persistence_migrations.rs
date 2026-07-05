crate::define_data_event_module!(
    PersistenceMigrationsCommand,
    PersistenceMigrationsOperation,
    append_persistence_migrations_event,
    "CODEX-0063-06-DATA-EVENTING-f6f824261f",
    "persistence_migrations",
    "PersistenceMigrationRecorded",
    "data_eventing.persistence_migrations.event_schema",
    crate::DataEventOperation::MigrationRecord,
    ["migration_ledger"]
);

pub const EVENT_STORE_MIGRATION_NAME: &str = "create_event_store";
pub const EVENT_OUTBOX_MIGRATION_NAME: &str = "create_event_outbox";
pub const PROJECTION_CHECKPOINT_MIGRATION_NAME: &str = "create_projection_checkpoint";

pub const EVENT_STORE_MIGRATION_SQL: &str = "\
CREATE TABLE event_store (
    sequence BIGSERIAL PRIMARY KEY,
    event_type TEXT NOT NULL,
    command_id TEXT NOT NULL,
    idempotency_key TEXT NOT NULL UNIQUE,
    expected_version BIGINT NOT NULL,
    authority_mode TEXT NOT NULL,
    authority_contract_version BIGINT NOT NULL,
    visibility_label TEXT NOT NULL,
    fact_provenance_kind TEXT NOT NULL,
    fact_provenance_reference TEXT NOT NULL,
    fact_recorded_by TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    causation_id TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT now()
);";

pub const EVENT_OUTBOX_MIGRATION_SQL: &str = "\
CREATE TABLE event_outbox (
    outbox_id BIGSERIAL PRIMARY KEY,
    event_sequence BIGINT NOT NULL,
    nats_subject TEXT NOT NULL,
    idempotency_key TEXT NOT NULL UNIQUE,
    visibility_label TEXT NOT NULL,
    correlation_id TEXT NOT NULL,
    causation_id TEXT NOT NULL,
    payload_json TEXT NOT NULL,
    published_at TIMESTAMPTZ,
    retry_count INTEGER NOT NULL DEFAULT 0
);";

pub const PROJECTION_CHECKPOINT_MIGRATION_SQL: &str = "\
CREATE TABLE projection_checkpoint (
    projection_name TEXT PRIMARY KEY,
    last_event_sequence BIGINT NOT NULL,
    projection_hash TEXT NOT NULL,
    rebuilt_at TIMESTAMPTZ NOT NULL DEFAULT now()
);";

pub const MIGRATION_STATEMENTS: &[(&str, &str)] = &[
    (EVENT_STORE_MIGRATION_NAME, EVENT_STORE_MIGRATION_SQL),
    (EVENT_OUTBOX_MIGRATION_NAME, EVENT_OUTBOX_MIGRATION_SQL),
    (
        PROJECTION_CHECKPOINT_MIGRATION_NAME,
        PROJECTION_CHECKPOINT_MIGRATION_SQL,
    ),
];

pub fn migration_statements() -> &'static [(&'static str, &'static str)] {
    MIGRATION_STATEMENTS
}
