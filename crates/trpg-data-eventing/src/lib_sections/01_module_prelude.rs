
pub use trpg_shared_kernel::{
    ActorOrigin, ActorRole, AgentClass, AuthorityContract, AuthorityMode, CommandEnvelope,
    EntityId, EventEnvelope, EventStore, FactProvenance, FormalWritePath, PrincipalScope,
    ProvenanceKind, TrpgError, Visibility, VisibilityLabel, WorkloadRole,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
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

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
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
        module_name: &'static str,
        event_type: &'static str,
        operation: DataEventOperation,
        projection_name: &'static str,
        event_schema_name: &'static str,
    ) -> Self {
        Self {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutboxDeliveryStatus {
    Pending,
    Claimed,
    Retrying,
    Published,
    DeadLettered,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutboxMessage<P> {
    pub event_id: u64,
    pub event_sequence: u64,
    pub campaign_id: EntityId,
    pub stream_id: EntityId,
    pub subject: &'static str,
    pub idempotency_key: String,
    pub visibility: Visibility,
    pub fact_provenance: FactProvenance,
    pub correlation_id: EntityId,
    pub causation_id: EntityId,
    pub payload: P,
    pub delivery_status: OutboxDeliveryStatus,
}

impl<P: Clone> From<&EventEnvelope<P>> for OutboxMessage<P> {
    fn from(event: &EventEnvelope<P>) -> Self {
        Self {
            event_id: event.sequence,
            event_sequence: event.sequence,
            campaign_id: event.campaign_id.clone(),
            stream_id: event.stream_id.clone(),
            subject: NATS_EVENTS_APPENDED,
            idempotency_key: event.idempotency_key.clone(),
            visibility: event.visibility.clone(),
            fact_provenance: event.fact_provenance.clone(),
            correlation_id: event.correlation_id.clone(),
            causation_id: event.causation_id.clone(),
            payload: event.payload.clone(),
            delivery_status: OutboxDeliveryStatus::Pending,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectionCheckpoint {
    pub projection_name: String,
    pub campaign_id: EntityId,
    pub stream_id: EntityId,
    pub version: u64,
    pub last_event_sequence: u64,
    pub projection_hash: String,
    pub rebuilt_at_unix_ms: u64,
}

impl ProjectionCheckpoint {
    pub fn from_snapshot(
        campaign_id: EntityId,
        stream_id: EntityId,
        stream_version: u64,
        snapshot: &ProjectionSnapshot,
    ) -> ProjectionCheckpoint {
        Self {
            projection_name: "data_event_projection".to_owned(),
            campaign_id,
            stream_id,
            version: stream_version,
            last_event_sequence: snapshot.last_sequence,
            projection_hash: snapshot.projection_hash.clone(),
            rebuilt_at_unix_ms: current_unix_time_ms(),
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
        adr_0002_event_sourcing_cqrs::contract(),
        adr_0002_event_sourcing_cqrs_event_sourcing_cqrs::contract(),
        adr_0004_nats_jetstream::contract(),
        adr_0005_postgres_pgvector::contract(),
        adr_0005_postgres_pgvector_postgre_sql_pgvector::contract(),
        adr_0010_rag_snapshot_rag_snapshot::contract(),
        event_json_schema_source_contract::contract(),
        event_json_schema::contract(),
        event_store_sqlx_outbox_projection::contract(),
        redis_cache_presence::contract(),
    ];
    contracts.extend(persistence_data_event_contracts());
    contracts.extend(transport_data_event_contracts());
    contracts
}

pub fn persistence_data_event_contracts() -> Vec<DataEventContract> {
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

pub fn transport_data_event_contracts() -> Vec<DataEventContract> {
    vec![
        api_websocket_nats_contracts::contract(),
        nats_subjects::contract(),
        nats_subject_contracts::contract(),
        nats_subjects_source_contract::contract(),
        domain_event_sourcing_projection::contract(),
        rag_snapshot::contract(),
        cache_redis_impl::contract(),
        event_bus_nats_impl::contract(),
        persistence_postgresql_impl::contract(),
    ]
}

pub fn schema_data_event_contracts() -> Vec<DataEventContract> {
    vec![event_json_schema::contract()]
}
