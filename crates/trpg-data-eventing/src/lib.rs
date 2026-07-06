pub mod adr_0002_event_sourcing_cqrs_event_sourcing_cqrs;
pub mod adr_0004_nats_jetstream;
pub mod adr_0005_postgres_pgvector_postgre_sql_pgvector;
pub mod adr_0010_rag_snapshot_rag_snapshot;
pub mod cache_redis;
pub mod database_schema_index;
pub mod event_bus_nats;
pub mod event_command_json_schema;
pub mod event_json_schema_source_contract;
pub mod event_schema_index;
pub mod event_sourcing_snapshot_projection;
pub mod event_store_projections;
pub mod event_store_sqlx_outbox_projection;
pub mod nats_jet_stream;
pub mod outbox_projection_workers;
pub mod persistence_migrations;
pub mod persistence_postgresql;
pub mod postgre_sql_sq_lx_pgvector;
pub mod readme;
pub mod redis_cache_presence;
pub mod redis_presence;
pub mod schema;
pub mod snapshot;
pub mod snapshot_strategy;
pub mod sqlx_migrations;
pub mod sqlx_migrations_contract;

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutboxMessage {
    pub event_id: u64,
    pub correlation_id: EntityId,
    pub causation_id: EntityId,
}

impl<P> From<&EventEnvelope<P>> for OutboxMessage {
    fn from(event: &EventEnvelope<P>) -> Self {
        Self {
            event_id: event.sequence,
            correlation_id: event.correlation_id.clone(),
            causation_id: event.causation_id.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectionCheckpoint {
    pub stream_id: EntityId,
    pub version: u64,
    pub projection_hash: String,
}

impl ProjectionCheckpoint {
    pub fn from_snapshot(
        stream_id: EntityId,
        snapshot: &ProjectionSnapshot,
    ) -> ProjectionCheckpoint {
        Self {
            stream_id,
            version: snapshot.last_sequence,
            projection_hash: snapshot.projection_hash.clone(),
        }
    }
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
    let hash = sha256_hex(&projection_hash_input(events));

    ProjectionSnapshot {
        event_count: events.len(),
        last_sequence,
        projection_hash: format!("sha256:{hash}"),
    }
}

pub fn replay_visible_data_events(
    store: &EventStore<DataEventPayload>,
    principal: &PrincipalScope,
) -> Vec<EventEnvelope<DataEventPayload>> {
    store.replay_visible(principal)
}

pub fn all_data_event_contracts() -> Vec<DataEventContract> {
    let mut contracts = vec![
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
    ];
    contracts.extend(batch_025_data_event_contracts());
    contracts
}

pub fn batch_025_data_event_contracts() -> Vec<DataEventContract> {
    vec![
        persistence_postgresql::contract(),
        redis_presence::contract(),
        nats_jet_stream::contract(),
        postgre_sql_sq_lx_pgvector::contract(),
        sqlx_migrations::contract(),
        event_sourcing_snapshot_projection::contract(),
        schema::contract(),
        readme::contract(),
        snapshot::contract(),
        event_command_json_schema::contract(),
        sqlx_migrations_contract::contract(),
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

fn projection_hash_input(events: &[EventEnvelope<DataEventPayload>]) -> Vec<u8> {
    let mut input = String::new();
    for event in events {
        input.push_str(&event.sequence.to_string());
        input.push('|');
        input.push_str(event.event_type);
        input.push('|');
        input.push_str(event.payload.module_name);
        input.push('|');
        input.push_str(event.payload.operation.as_str());
        input.push('\n');
    }
    input.into_bytes()
}

fn sha256_hex(input: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    let bit_len = (input.len() as u64) * 8;
    let mut message = input.to_vec();
    message.push(0x80);
    while (message.len() % 64) != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_len.to_be_bytes());

    let mut state = [
        0x6a09e667u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];

    let mut words = [0u32; 64];
    for chunk in message.chunks_exact(64) {
        for (index, word) in words.iter_mut().take(16).enumerate() {
            let start = index * 4;
            *word = u32::from_be_bytes([
                chunk[start],
                chunk[start + 1],
                chunk[start + 2],
                chunk[start + 3],
            ]);
        }
        for index in 16..64 {
            let s0 = words[index - 15].rotate_right(7)
                ^ words[index - 15].rotate_right(18)
                ^ (words[index - 15] >> 3);
            let s1 = words[index - 2].rotate_right(17)
                ^ words[index - 2].rotate_right(19)
                ^ (words[index - 2] >> 10);
            words[index] = words[index - 16]
                .wrapping_add(s0)
                .wrapping_add(words[index - 7])
                .wrapping_add(s1);
        }

        let mut a = state[0];
        let mut b = state[1];
        let mut c = state[2];
        let mut d = state[3];
        let mut e = state[4];
        let mut f = state[5];
        let mut g = state[6];
        let mut h = state[7];

        for index in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ ((!e) & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[index])
                .wrapping_add(words[index]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
        state[4] = state[4].wrapping_add(e);
        state[5] = state[5].wrapping_add(f);
        state[6] = state[6].wrapping_add(g);
        state[7] = state[7].wrapping_add(h);
    }

    state
        .iter()
        .map(|word| format!("{word:08x}"))
        .collect::<String>()
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

#[macro_export]
macro_rules! define_data_event_artifacts {
    (
        $service:ident,
        $repository:ident,
        $event:ident,
        $error:ident,
        $event_type:ident,
        $schema_name:ident
    ) => {
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
        pub struct $service;

        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
        pub struct $repository;

        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub struct $event {
            pub event_type: &'static str,
            pub schema_name: &'static str,
        }

        impl $event {
            pub const fn new() -> Self {
                Self {
                    event_type: $event_type,
                    schema_name: $schema_name,
                }
            }
        }

        impl Default for $event {
            fn default() -> Self {
                Self::new()
            }
        }

        #[derive(Clone, Debug, PartialEq, Eq)]
        pub enum $error {
            GovernanceViolation(&'static str),
        }
    };
}
