pub mod adr_0002_event_sourcing_cqrs_event_sourcing_cqrs;
pub mod adr_0004_nats_jetstream;
pub mod adr_0005_postgres_pgvector_postgre_sql_pgvector;
pub mod adr_0010_rag_snapshot_rag_snapshot;
pub mod cache_redis;
pub mod database_schema_index;
pub mod event_bus_nats;
pub mod event_json_schema_source_contract;
pub mod event_schema_index;
pub mod event_store_projections;
pub mod event_store_sqlx_outbox_projection;
pub mod outbox_projection_workers;
pub mod persistence_migrations;
pub mod redis_cache_presence;
pub mod snapshot_strategy;

pub use trpg_shared_kernel::{
    ActorRole, AuthorityContract, AuthorityMode, CommandEnvelope, EntityId, EventEnvelope,
    EventStore, FactProvenance, FormalWritePath, PrincipalScope, ProvenanceKind, TrpgError,
    Visibility, VisibilityLabel,
};

pub type DataEventResult<T> = Result<T, TrpgError>;

pub const EVENT_STORE_TABLE: &str = "event_store";
pub const OUTBOX_TABLE: &str = "event_outbox";
pub const NATS_EVENTS_APPENDED: &str = "trpg.events.appended";
pub const NATS_PROJECTION_REBUILD_REQUESTED: &str = "trpg.projection.rebuild.requested";
pub const COMMAND_ENVELOPE_REQUIRED_FIELDS: &[&str] = &[
    "command_id",
    "idempotency_key",
    "expected_version",
    "actor",
    "authority_mode",
    "authority_contract_version",
    "visibility",
    "fact_provenance",
    "correlation_id",
    "causation_id",
    "write_path",
];
pub const EVENT_ENVELOPE_REQUIRED_FIELDS: &[&str] = &[
    "sequence",
    "event_type",
    "command_id",
    "idempotency_key",
    "authority_contract_version",
    "visibility",
    "fact_provenance",
    "correlation_id",
    "causation_id",
    "payload",
];
pub const DATA_EVENT_NATS_SUBJECTS: &[&str] =
    &[NATS_EVENTS_APPENDED, NATS_PROJECTION_REBUILD_REQUESTED];
pub const DATA_EVENT_METRICS: &[&str] = &[
    "trpg_command_total",
    "trpg_event_append_latency_ms",
    "trpg_policy_deny_total",
    "trpg_projection_lag_events",
    "trpg_visibility_redaction_total",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataEventOperation {
    EventStoreAppend,
    OutboxPublish,
    ProjectionRebuild,
    SchemaRegister,
    MigrationRecord,
    SnapshotCreate,
    CacheWrite,
    ArchitectureDecisionRecord,
}

impl DataEventOperation {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::EventStoreAppend => "event_store_append",
            Self::OutboxPublish => "outbox_publish",
            Self::ProjectionRebuild => "projection_rebuild",
            Self::SchemaRegister => "schema_register",
            Self::MigrationRecord => "migration_record",
            Self::SnapshotCreate => "snapshot_create",
            Self::CacheWrite => "cache_write",
            Self::ArchitectureDecisionRecord => "architecture_decision_record",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataEventPayload {
    pub module_name: &'static str,
    pub event_name: &'static str,
    pub operation: DataEventOperation,
    pub read_models: &'static [&'static str],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DataEventWrite {
    pub module_name: &'static str,
    pub event_type: &'static str,
    pub operation: DataEventOperation,
    pub read_models: &'static [&'static str],
}

impl DataEventWrite {
    pub const fn new(
        module_name: &'static str,
        event_type: &'static str,
        operation: DataEventOperation,
        read_models: &'static [&'static str],
    ) -> Self {
        Self {
            module_name,
            event_type,
            operation,
            read_models,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DataEventContract {
    pub prompt_id: &'static str,
    pub module_name: &'static str,
    pub event_type: &'static str,
    pub operation: DataEventOperation,
    pub event_store_table: &'static str,
    pub outbox_table: &'static str,
    pub projection_name: &'static str,
    pub event_schema_name: &'static str,
    pub nats_subjects: &'static [&'static str],
    pub metrics: &'static [&'static str],
    pub required_command_fields: &'static [&'static str],
    pub required_event_fields: &'static [&'static str],
    pub canon_boundary: &'static str,
}

impl DataEventContract {
    pub fn new(
        prompt_id: &'static str,
        module_name: &'static str,
        event_type: &'static str,
        operation: DataEventOperation,
        projection_name: &'static str,
        event_schema_name: &'static str,
    ) -> Self {
        Self {
            prompt_id,
            module_name,
            event_type,
            operation,
            event_store_table: EVENT_STORE_TABLE,
            outbox_table: OUTBOX_TABLE,
            projection_name,
            event_schema_name,
            nats_subjects: DATA_EVENT_NATS_SUBJECTS,
            metrics: DATA_EVENT_METRICS,
            required_command_fields: COMMAND_ENVELOPE_REQUIRED_FIELDS,
            required_event_fields: EVENT_ENVELOPE_REQUIRED_FIELDS,
            canon_boundary:
                "formal_facts_only_through_command_workflow_decision_event_store_projection",
        }
    }

    pub fn uses_current_safe_names(&self) -> bool {
        [
            self.module_name,
            self.event_type,
            self.event_store_table,
            self.outbox_table,
            self.projection_name,
            self.event_schema_name,
        ]
        .iter()
        .all(|value| is_current_safe_name(value))
            && self
                .nats_subjects
                .iter()
                .all(|value| is_current_safe_name(value))
            && self.metrics.iter().all(|value| is_current_safe_name(value))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectionSnapshot {
    pub event_count: usize,
    pub last_sequence: u64,
    pub projection_hash: String,
}

pub fn append_data_event<T>(
    store: &mut EventStore<DataEventPayload>,
    contract: &AuthorityContract,
    command: &CommandEnvelope<T>,
    write: DataEventWrite,
) -> DataEventResult<EventEnvelope<DataEventPayload>> {
    if !is_current_safe_name(write.module_name) || !is_current_safe_name(write.event_type) {
        return Err(TrpgError::CodingPolicyViolation(
            "data_eventing_current_safe_name",
        ));
    }

    contract.validate_command(command)?;
    store.append(
        command,
        write.event_type,
        DataEventPayload {
            module_name: write.module_name,
            event_name: write.event_type,
            operation: write.operation,
            read_models: write.read_models,
        },
    )
}

pub fn rebuild_projection_from_events(
    events: &[EventEnvelope<DataEventPayload>],
) -> ProjectionSnapshot {
    let last_sequence = events.last().map(|event| event.sequence).unwrap_or(0);
    let mut hash = 0xcbf29ce484222325u64;

    for event in events {
        fold_bytes(&mut hash, event.event_type.as_bytes());
        fold_bytes(&mut hash, event.payload.module_name.as_bytes());
        fold_bytes(&mut hash, event.payload.operation.as_str().as_bytes());
        fold_bytes(&mut hash, event.sequence.to_string().as_bytes());
    }

    ProjectionSnapshot {
        event_count: events.len(),
        last_sequence,
        projection_hash: format!("{hash:016x}"),
    }
}

pub fn replay_visible_data_events(
    store: &EventStore<DataEventPayload>,
    principal: &PrincipalScope,
) -> Vec<EventEnvelope<DataEventPayload>> {
    store.replay_visible(principal)
}

pub fn all_data_event_contracts() -> Vec<DataEventContract> {
    vec![
        cache_redis::contract(),
        database_schema_index::contract(),
        event_bus_nats::contract(),
        event_schema_index::contract(),
        event_store_projections::contract(),
        outbox_projection_workers::contract(),
        persistence_migrations::contract(),
        snapshot_strategy::contract(),
        adr_0002_event_sourcing_cqrs_event_sourcing_cqrs::contract(),
        adr_0004_nats_jetstream::contract(),
        adr_0005_postgres_pgvector_postgre_sql_pgvector::contract(),
        adr_0010_rag_snapshot_rag_snapshot::contract(),
        event_json_schema_source_contract::contract(),
        event_store_sqlx_outbox_projection::contract(),
        redis_cache_presence::contract(),
    ]
}

pub fn is_current_safe_name(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return false;
    }

    let lower = trimmed.to_ascii_lowercase();
    let denied = [
        "generated-from-source",
        "generated_from_source",
        "source-breakdow",
        "source_breakdow",
        "docs-implementation",
        "docs_implementation",
        "implementation-90",
        "implementation_90",
        "fix-history",
        "fix_history",
        "legacy",
        "v3",
        "v4",
        "v5",
        "v6",
    ];

    if denied.iter().any(|token| lower.contains(token)) {
        return false;
    }

    trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
        && !has_long_hex_run(trimmed)
}

fn has_long_hex_run(value: &str) -> bool {
    let mut run = 0;
    for ch in value.chars() {
        if ch.is_ascii_hexdigit() {
            run += 1;
            if run >= 10 {
                return true;
            }
        } else {
            run = 0;
        }
    }

    false
}

fn fold_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x100000001b3);
    }
}

#[macro_export]
macro_rules! define_data_event_module {
    (
        $command:ident,
        $operation_ty:ident,
        $append_fn:ident,
        $prompt_id:literal,
        $module_name:literal,
        $event_type:literal,
        $schema_name:literal,
        $operation_kind:expr,
        [$($read_model:literal),* $(,)?]
    ) => {
        pub const PROMPT_ID: &str = $prompt_id;
        pub const MODULE_NAME: &str = $module_name;
        pub const EVENT_TYPE: &str = $event_type;
        pub const EVENT_SCHEMA_NAME: &str = $schema_name;
        pub const READ_MODELS: &[&str] = &[$($read_model),*];

        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct $command {
            pub operation: $operation_ty,
            pub reason: &'static str,
        }

        impl $command {
            pub fn record(reason: &'static str) -> Self {
                Self {
                    operation: $operation_ty::RecordGovernedChange,
                    reason,
                }
            }
        }

        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum $operation_ty {
            RecordGovernedChange,
        }

        pub fn $append_fn<T>(
            store: &mut trpg_shared_kernel::EventStore<$crate::DataEventPayload>,
            contract: &trpg_shared_kernel::AuthorityContract,
            command: &trpg_shared_kernel::CommandEnvelope<T>,
        ) -> $crate::DataEventResult<trpg_shared_kernel::EventEnvelope<$crate::DataEventPayload>> {
            $crate::append_data_event(
                store,
                contract,
                command,
                $crate::DataEventWrite::new(
                    MODULE_NAME,
                    EVENT_TYPE,
                    $operation_kind,
                    READ_MODELS,
                ),
            )
        }

        pub fn contract() -> $crate::DataEventContract {
            $crate::DataEventContract::new(
                PROMPT_ID,
                MODULE_NAME,
                EVENT_TYPE,
                $operation_kind,
                MODULE_NAME,
                EVENT_SCHEMA_NAME,
            )
        }
    };
}
