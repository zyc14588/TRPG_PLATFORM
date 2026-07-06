use std::collections::HashSet;

use trpg_data_eventing::{
    batch_025_data_event_contracts, event_command_json_schema, event_sourcing_snapshot_projection,
    is_current_safe_name, nats_jet_stream, persistence_migrations, persistence_postgresql,
    postgre_sql_sq_lx_pgvector, rebuild_projection_from_events, schema, snapshot, sqlx_migrations,
    sqlx_migrations_contract, ActorRole, AuthorityContract, AuthorityMode, CommandEnvelope,
    DataEventOperation, DataEventPayload, EventStore, FactProvenance, FormalWritePath,
    ProvenanceKind, TrpgError, COMMAND_ENVELOPE_REQUIRED_FIELDS, EVENT_ENVELOPE_REQUIRED_FIELDS,
    EVENT_STORE_TABLE, NATS_EVENTS_APPENDED, NATS_PROJECTION_REBUILD_REQUESTED, OUTBOX_TABLE,
};

#[test]
fn b025_primary_contracts_map_to_current_safe_outputs() {
    let contracts = batch_025_data_event_contracts();
    assert_eq!(contracts.len(), 11);

    let expected = [
        (
            "CODEX-0601-06-DATA-EVENTING-6b440dcd4b",
            "persistence_postgresql",
            DataEventOperation::EventStoreAppend,
        ),
        (
            "CODEX-0603-06-DATA-EVENTING-dd4ec4ebfa",
            "redis_presence",
            DataEventOperation::CacheWrite,
        ),
        (
            "CODEX-0604-06-DATA-EVENTING-2cd43712b5",
            "nats_jet_stream",
            DataEventOperation::OutboxPublish,
        ),
        (
            "CODEX-0605-06-DATA-EVENTING-7aa50c4023",
            "postgre_sql_sq_lx_pgvector",
            DataEventOperation::ProjectionRebuild,
        ),
        (
            "CODEX-0606-06-DATA-EVENTING-96df5cfdb1",
            "sqlx_migrations",
            DataEventOperation::MigrationRecord,
        ),
        (
            "CODEX-0607-06-DATA-EVENTING-2a432fa185",
            "event_sourcing_snapshot_projection",
            DataEventOperation::ProjectionRebuild,
        ),
        (
            "CODEX-0609-06-DATA-EVENTING-6f17ea580b",
            "schema",
            DataEventOperation::SchemaRegister,
        ),
        (
            "CODEX-0615-06-DATA-EVENTING-58af1867fc",
            "readme",
            DataEventOperation::ArchitectureDecisionRecord,
        ),
        (
            "CODEX-0616-06-DATA-EVENTING-0b28c2b885",
            "snapshot",
            DataEventOperation::SnapshotCreate,
        ),
        (
            "CODEX-0620-06-DATA-EVENTING-f991a07544",
            "event_command_json_schema",
            DataEventOperation::SchemaRegister,
        ),
        (
            "CODEX-0625-06-DATA-EVENTING-181b11b4cd",
            "sqlx_migrations_contract",
            DataEventOperation::MigrationRecord,
        ),
    ];

    let modules: HashSet<_> = contracts
        .iter()
        .map(|contract| contract.module_name)
        .collect();
    assert_eq!(modules.len(), contracts.len());

    for (prompt_id, module_name, operation) in expected {
        let contract = contracts
            .iter()
            .find(|contract| contract.prompt_id == prompt_id)
            .expect("B025 primary prompt has a contract");

        assert_eq!(contract.module_name, module_name);
        assert_eq!(contract.operation, operation);
        assert_eq!(contract.event_store_table, EVENT_STORE_TABLE);
        assert_eq!(contract.outbox_table, OUTBOX_TABLE);
        assert!(contract.uses_current_safe_names());
        assert!(contract.nats_subjects.contains(&NATS_EVENTS_APPENDED));
        assert!(contract
            .nats_subjects
            .contains(&NATS_PROJECTION_REBUILD_REQUESTED));
        assert!(contract
            .required_command_fields
            .contains(&"idempotency_key"));
        assert!(contract
            .required_command_fields
            .contains(&"expected_version"));
        assert!(contract.required_event_fields.contains(&"visibility"));
        assert!(contract.required_event_fields.contains(&"fact_provenance"));
    }
}

#[test]
fn b025_appends_only_through_governed_event_store_path() {
    let authority_contract = authority_contract(AuthorityMode::AiKp);
    let mut store: EventStore<DataEventPayload> = EventStore::default();

    let first = governed_command(
        persistence_postgresql::PersistencePostgresqlCommand::record("postgres append"),
        0,
        "idem_b025_postgres",
        ActorRole::Workflow,
        AuthorityMode::AiKp,
    );
    let event = persistence_postgresql::append_persistence_postgresql_event(
        &mut store,
        &authority_contract,
        &first,
    )
    .unwrap();

    assert_eq!(event.sequence, 1);
    assert_eq!(
        event.payload.module_name,
        persistence_postgresql::MODULE_NAME
    );
    assert_eq!(event.fact_provenance.reference.as_str(), "fact_b025");

    let stale_version = governed_command(
        persistence_postgresql::PersistencePostgresqlCommand::record("stale write"),
        0,
        "idem_b025_stale",
        ActorRole::Workflow,
        AuthorityMode::AiKp,
    );
    let error = persistence_postgresql::append_persistence_postgresql_event(
        &mut store,
        &authority_contract,
        &stale_version,
    )
    .unwrap_err();
    assert_eq!(
        error,
        TrpgError::ExpectedVersionConflict {
            expected: 0,
            actual: 1,
        }
    );

    let duplicate = governed_command(
        persistence_postgresql::PersistencePostgresqlCommand::record("duplicate"),
        1,
        "idem_b025_postgres",
        ActorRole::Workflow,
        AuthorityMode::AiKp,
    );
    let error = persistence_postgresql::append_persistence_postgresql_event(
        &mut store,
        &authority_contract,
        &duplicate,
    )
    .unwrap_err();
    assert_eq!(error.code(), "DUPLICATE_COMMAND");

    let mut direct_agent = governed_command(
        nats_jet_stream::NatsJetStreamCommand::record("agent bypass"),
        1,
        "idem_b025_direct_agent",
        ActorRole::Workflow,
        AuthorityMode::AiKp,
    );
    direct_agent.write_path = FormalWritePath::DirectAgent;
    let error = nats_jet_stream::append_nats_jet_stream_event(
        &mut store,
        &authority_contract,
        &direct_agent,
    )
    .unwrap_err();
    assert_eq!(error.code(), "DIRECT_AGENT_STATE_WRITE");
    assert_eq!(store.events().len(), 1);
}

#[test]
fn b025_projection_snapshot_and_rag_outputs_are_rebuildable_read_models() {
    let authority_contract = authority_contract(AuthorityMode::AiKp);
    let mut store: EventStore<DataEventPayload> = EventStore::default();

    event_sourcing_snapshot_projection::append_event_sourcing_snapshot_projection_event(
        &mut store,
        &authority_contract,
        &governed_command(
            event_sourcing_snapshot_projection::EventSourcingSnapshotProjectionCommand::record(
                "projection rebuild",
            ),
            0,
            "idem_b025_projection",
            ActorRole::Workflow,
            AuthorityMode::AiKp,
        ),
    )
    .unwrap();
    snapshot::append_snapshot_event(
        &mut store,
        &authority_contract,
        &governed_command(
            snapshot::SnapshotCommand::record("snapshot read model"),
            1,
            "idem_b025_snapshot",
            ActorRole::Workflow,
            AuthorityMode::AiKp,
        ),
    )
    .unwrap();
    postgre_sql_sq_lx_pgvector::append_postgre_sql_sq_lx_pgvector_event(
        &mut store,
        &authority_contract,
        &governed_command(
            postgre_sql_sq_lx_pgvector::PostgreSqlSqLxPgvectorCommand::record("rag index"),
            2,
            "idem_b025_pgvector",
            ActorRole::Workflow,
            AuthorityMode::AiKp,
        ),
    )
    .unwrap();

    let first = rebuild_projection_from_events(store.events());
    let second = rebuild_projection_from_events(store.events());
    assert_eq!(first, second);
    assert_eq!(first.event_count, 3);
    assert_eq!(first.last_sequence, 3);
    assert_ne!(first.projection_hash, "0000000000000000");
    assert!(event_sourcing_snapshot_projection::READ_MODELS.contains(&"event_store"));
    assert_eq!(snapshot::SNAPSHOT_REBUILD_SOURCE, "event_store");
    assert!(postgre_sql_sq_lx_pgvector::RAG_INDEX_FIELDS.contains(&"visibility"));
    assert!(postgre_sql_sq_lx_pgvector::RAG_INDEX_FIELDS.contains(&"fact_provenance"));
}

#[test]
fn b025_schema_and_migration_contracts_preserve_required_metadata() {
    for required in [
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
    ] {
        assert!(event_command_json_schema::command_schema_fields().contains(&required));
        assert!(schema::required_command_fields().contains(&required));
        assert!(COMMAND_ENVELOPE_REQUIRED_FIELDS.contains(&required));
    }

    for required in [
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
    ] {
        assert!(event_command_json_schema::event_schema_fields().contains(&required));
        assert!(schema::required_event_fields().contains(&required));
        assert!(EVENT_ENVELOPE_REQUIRED_FIELDS.contains(&required));
    }

    let migration_text = sqlx_migrations::migration_statements()
        .iter()
        .map(|(_, sql)| *sql)
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        sqlx_migrations::migration_statements(),
        persistence_migrations::migration_statements()
    );
    for column in sqlx_migrations_contract::required_event_store_columns() {
        assert!(migration_text.contains(column));
        assert!(is_current_safe_name(column));
    }

    for (name, sql) in sqlx_migrations::migration_statements() {
        assert!(is_current_safe_name(name));
        assert!(sql.contains("CREATE TABLE"));
        assert!(!sql.contains("generated-from-source"));
        assert!(!sql.contains("v6"));
    }
}

#[test]
fn b025_domain_artifacts_are_named_and_schema_bound() {
    let persistence_event = persistence_postgresql::PersistencePostgresqlEvent::new();
    assert_eq!(
        persistence_event.event_type,
        persistence_postgresql::EVENT_TYPE
    );
    assert_eq!(
        persistence_event.schema_name,
        persistence_postgresql::EVENT_SCHEMA_NAME
    );

    let schema_event = event_command_json_schema::EventCommandJsonSchemaEvent::new();
    assert_eq!(
        schema_event.event_type,
        event_command_json_schema::EVENT_TYPE
    );
    assert_eq!(
        schema_event.schema_name,
        event_command_json_schema::EVENT_SCHEMA_NAME
    );
}

fn authority_contract(mode: AuthorityMode) -> AuthorityContract {
    AuthorityContract::new("campaign_batch_025", mode, 1).unwrap()
}

fn governed_command<T>(
    payload: T,
    expected_version: u64,
    idempotency_key: &str,
    role: ActorRole,
    mode: AuthorityMode,
) -> CommandEnvelope<T> {
    let mut command = CommandEnvelope::governed(payload, role, mode);
    command.idempotency_key = idempotency_key.to_owned();
    command.expected_version = expected_version;
    command.fact_provenance =
        FactProvenance::new(ProvenanceKind::SystemFixture, "fact_b025", "batch_025").unwrap();
    command
}
