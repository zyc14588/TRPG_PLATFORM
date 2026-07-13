use std::collections::HashSet;

use trpg_data_eventing::{
    all_data_event_contracts, cache_redis, database_schema_index, event_bus_nats,
    event_store_projections, is_current_safe_name, persistence_migrations,
    rebuild_projection_from_events, replay_visible_data_events, DataEventPayload, EventStore,
    FactProvenance, FormalWritePath, PrincipalScope, ProvenanceKind, TrpgError, Visibility,
    VisibilityLabel, COMMAND_ENVELOPE_REQUIRED_FIELDS, EVENT_ENVELOPE_REQUIRED_FIELDS,
    EVENT_STORE_TABLE, NATS_EVENTS_APPENDED, NATS_PROJECTION_REBUILD_REQUESTED, OUTBOX_TABLE,
};
use trpg_data_eventing::{ActorRole, AuthorityContract, AuthorityMode, CommandEnvelope, EntityId};

#[test]
fn b024_contracts_map_all_modules_to_current_safe_outputs() {
    let contracts = all_data_event_contracts();
    assert!(contracts.len() >= 15);

    let expected = [
        "cache_redis",
        "database_schema_index",
        "event_bus_nats",
        "event_schema_index",
        "event_store_projections",
        "outbox_projection_workers",
        "persistence_migrations",
        "snapshot_strategy",
        "adr_0002_event_sourcing_cqrs_event_sourcing_cqrs",
        "adr_0004_nats_jetstream",
        "adr_0005_postgres_pgvector_postgre_sql_pgvector",
        "adr_0010_rag_snapshot_rag_snapshot",
        "event_json_schema_source_contract",
        "event_store_sqlx_outbox_projection",
        "redis_cache_presence",
    ];

    let modules: HashSet<_> = contracts
        .iter()
        .map(|contract| contract.module_name)
        .collect();
    assert_eq!(modules.len(), contracts.len());

    for module_name in expected {
        let contract = contracts
            .iter()
            .find(|contract| contract.module_name == module_name)
            .expect("B024 module has a contract");

        assert_eq!(contract.module_name, module_name);
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
        assert!(contract
            .required_command_fields
            .contains(&"fact_provenance"));
        assert!(contract.required_event_fields.contains(&"visibility"));
        assert!(contract.required_event_fields.contains(&"correlation_id"));
    }
}

#[test]
fn b024_rejects_legacy_or_source_derived_names() {
    for name in [
        "docs_implementation_06_data_eventing_event_bus_nats_v4",
        "generated-from-source-event-store",
        "cache_redis_554c39e311",
        "legacy_projection_worker",
    ] {
        assert!(!is_current_safe_name(name));
    }

    for contract in all_data_event_contracts() {
        assert!(is_current_safe_name(contract.module_name));
        assert!(is_current_safe_name(contract.event_type));
        assert!(is_current_safe_name(contract.event_schema_name));
    }
}

#[test]
fn b024_appends_formal_events_through_contract_and_preserves_replay_metadata() {
    let contract = authority_contract(AuthorityMode::AiKp);
    let command_payload =
        event_store_projections::EventStoreProjectionsCommand::record("projection replay");
    let mut command = governed_command(
        command_payload,
        0,
        "idem_projection_001",
        ActorRole::Workflow,
        AuthorityMode::AiKp,
    );
    let provenance = FactProvenance::new(
        ProvenanceKind::ToolResult,
        "fact_projection_001",
        "tool_001",
    )
    .unwrap();
    command.visibility = Visibility::new(VisibilityLabel::KeeperOnly);
    command.fact_provenance = provenance.clone();

    let mut store: EventStore<DataEventPayload> = EventStore::default();
    let event = event_store_projections::append_event_store_projections_event(
        &mut store, &contract, &command,
    )
    .unwrap();
    let snapshot = rebuild_projection_from_events(store.events());

    assert_eq!(event.sequence, 1);
    assert_eq!(
        event.payload.module_name,
        event_store_projections::MODULE_NAME
    );
    assert_eq!(
        event.payload.event_name,
        event_store_projections::EVENT_TYPE
    );
    assert_eq!(event.fact_provenance, provenance);
    assert_eq!(
        event.visibility,
        Visibility::new(VisibilityLabel::KeeperOnly)
    );
    assert_eq!(snapshot.event_count, 1);
    assert_eq!(snapshot.last_sequence, 1);
    assert_ne!(snapshot.projection_hash, "0000000000000000");
    assert_eq!(
        replay_visible_data_events(&store, &PrincipalScope::Keeper).len(),
        1
    );
    assert!(replay_visible_data_events(
        &store,
        &PrincipalScope::Player(EntityId::new("player_001").unwrap())
    )
    .is_empty());
}

#[test]
fn b024_blocks_direct_agent_business_and_authority_contract_bypass() {
    let contract = authority_contract(AuthorityMode::AiKp);
    let mut store: EventStore<DataEventPayload> = EventStore::default();

    let mut direct_agent = governed_command(
        cache_redis::CacheRedisCommand::record("agent cannot write cache canon"),
        0,
        "idem_direct_agent",
        ActorRole::Workflow,
        AuthorityMode::AiKp,
    );
    direct_agent.write_path = FormalWritePath::DirectAgent;
    let error =
        cache_redis::append_cache_redis_event(&mut store, &contract, &direct_agent).unwrap_err();
    assert_eq!(error.code(), "DIRECT_AGENT_STATE_WRITE");

    let mut direct_business = governed_command(
        database_schema_index::DatabaseSchemaIndexCommand::record("business cannot write canon"),
        0,
        "idem_direct_business",
        ActorRole::Workflow,
        AuthorityMode::AiKp,
    );
    direct_business.write_path = FormalWritePath::DirectBusiness;
    let error = database_schema_index::append_database_schema_index_event(
        &mut store,
        &contract,
        &direct_business,
    )
    .unwrap_err();
    assert_eq!(error.code(), "POLICY_DENIED");

    let bad_actor = governed_command(
        cache_redis::CacheRedisCommand::record("human keeper cannot write AI_KP canon"),
        0,
        "idem_bad_actor",
        ActorRole::HumanKeeper,
        AuthorityMode::AiKp,
    );
    let error =
        cache_redis::append_cache_redis_event(&mut store, &contract, &bad_actor).unwrap_err();
    assert_eq!(error.code(), "AUTHORITY_VIOLATION");

    let mut mutated_contract_version = governed_command(
        cache_redis::CacheRedisCommand::record("contract version cannot mutate in place"),
        0,
        "idem_mutated_contract",
        ActorRole::Workflow,
        AuthorityMode::AiKp,
    );
    mutated_contract_version.authority_contract_version = 2;
    let error =
        cache_redis::append_cache_redis_event(&mut store, &contract, &mutated_contract_version)
            .unwrap_err();
    assert_eq!(error.code(), "AUTHORITY_CONTRACT_MUTATION");
    assert!(store.events().is_empty());
}

#[test]
fn b024_enforces_expected_version_and_idempotency_for_event_store_canon() {
    let contract = authority_contract(AuthorityMode::AiKp);
    let mut store: EventStore<DataEventPayload> = EventStore::default();

    let first = governed_command(
        event_bus_nats::EventBusNatsCommand::record("publish from outbox only"),
        0,
        "idem_nats_001",
        ActorRole::Workflow,
        AuthorityMode::AiKp,
    );
    event_bus_nats::append_event_bus_nats_event(&mut store, &contract, &first).unwrap();

    let stale_expected_version = governed_command(
        event_bus_nats::EventBusNatsCommand::record("stale expected version"),
        0,
        "idem_nats_002",
        ActorRole::Workflow,
        AuthorityMode::AiKp,
    );
    let error =
        event_bus_nats::append_event_bus_nats_event(&mut store, &contract, &stale_expected_version)
            .unwrap_err();
    assert_eq!(
        error,
        TrpgError::ExpectedVersionConflict {
            expected: 0,
            actual: 1,
        }
    );

    let duplicate = governed_command(
        event_bus_nats::EventBusNatsCommand::record("duplicate idempotency key"),
        1,
        "idem_nats_001",
        ActorRole::Workflow,
        AuthorityMode::AiKp,
    );
    let error =
        event_bus_nats::append_event_bus_nats_event(&mut store, &contract, &duplicate).unwrap_err();
    assert_eq!(error.code(), "DUPLICATE_COMMAND");
    assert_eq!(store.events().len(), 1);
}

#[test]
fn b024_redacts_private_keeper_and_ai_internal_from_player_visible_replay() {
    let contract = authority_contract(AuthorityMode::AiKp);
    let mut store: EventStore<DataEventPayload> = EventStore::default();
    let player_a = EntityId::new("player_a").unwrap();
    let player_b = EntityId::new("player_b").unwrap();

    let mut private_to_a = governed_command(
        cache_redis::CacheRedisCommand::record("private player cache delta"),
        0,
        "idem_private_player_a",
        ActorRole::Workflow,
        AuthorityMode::AiKp,
    );
    private_to_a.visibility = Visibility::private_to_player(player_a.clone());
    cache_redis::append_cache_redis_event(&mut store, &contract, &private_to_a).unwrap();

    let mut keeper_only = governed_command(
        event_store_projections::EventStoreProjectionsCommand::record("keeper projection note"),
        1,
        "idem_keeper_projection",
        ActorRole::Workflow,
        AuthorityMode::AiKp,
    );
    keeper_only.visibility = Visibility::new(VisibilityLabel::KeeperOnly);
    event_store_projections::append_event_store_projections_event(
        &mut store,
        &contract,
        &keeper_only,
    )
    .unwrap();

    let mut ai_internal = governed_command(
        event_bus_nats::EventBusNatsCommand::record("ai internal outbox trace"),
        2,
        "idem_ai_internal",
        ActorRole::Workflow,
        AuthorityMode::AiKp,
    );
    ai_internal.visibility = Visibility::new(VisibilityLabel::AiInternal);
    event_bus_nats::append_event_bus_nats_event(&mut store, &contract, &ai_internal).unwrap();

    assert_eq!(
        replay_visible_data_events(&store, &PrincipalScope::Player(player_a)).len(),
        1
    );
    assert!(replay_visible_data_events(&store, &PrincipalScope::Player(player_b)).is_empty());
    assert_eq!(
        replay_visible_data_events(&store, &PrincipalScope::Keeper).len(),
        2
    );
    assert_eq!(
        replay_visible_data_events(&store, &PrincipalScope::System).len(),
        3
    );
    assert!(replay_visible_data_events(&store, &PrincipalScope::Public).is_empty());
}

#[test]
fn b024_declares_required_command_event_schema_fields() {
    for required in [
        "command_id",
        "idempotency_key",
        "expected_version",
        "actor",
        "authority_mode",
        "visibility",
        "fact_provenance",
        "correlation_id",
        "causation_id",
        "write_path",
    ] {
        assert!(COMMAND_ENVELOPE_REQUIRED_FIELDS.contains(&required));
    }

    for required in [
        "sequence",
        "event_type",
        "command_id",
        "idempotency_key",
        "visibility",
        "fact_provenance",
        "correlation_id",
        "causation_id",
        "payload",
    ] {
        assert!(EVENT_ENVELOPE_REQUIRED_FIELDS.contains(&required));
    }
}

#[test]
fn b024_declares_current_safe_sqlx_migration_contract() {
    let statements = persistence_migrations::migration_statements();
    assert_eq!(statements.len(), 3);

    for (name, sql) in statements {
        assert!(is_current_safe_name(name));
        assert!(!sql.contains("generated-from-source"));
        assert!(!sql.contains("v4"));
        assert!(!sql.contains("v5"));
    }

    let event_store_sql = persistence_migrations::EVENT_STORE_MIGRATION_SQL;
    for required in [
        "event_store",
        "idempotency_key",
        "expected_version",
        "authority_contract_version",
        "visibility_label",
        "fact_provenance_kind",
        "correlation_id",
        "causation_id",
        "UNIQUE",
    ] {
        assert!(event_store_sql.contains(required));
    }

    let outbox_sql = persistence_migrations::EVENT_OUTBOX_MIGRATION_SQL;
    for required in [
        "event_outbox",
        "event_sequence",
        "nats_subject",
        "idempotency_key",
        "visibility_label",
        "published_at",
        "retry_count",
    ] {
        assert!(outbox_sql.contains(required));
    }
}

fn authority_contract(mode: AuthorityMode) -> AuthorityContract {
    AuthorityContract::new("campaign_data_eventing_001", mode, 1).unwrap()
}

fn governed_command<T>(
    payload: T,
    expected_version: u64,
    idempotency_key: &str,
    role: ActorRole,
    mode: AuthorityMode,
) -> CommandEnvelope<T> {
    let mut command = trpg_test_support::governed_command!(payload, role, mode);
    command.command_id = EntityId::new(format!("command_{idempotency_key}")).unwrap();
    command.idempotency_key = idempotency_key.to_owned();
    command.expected_version = expected_version;
    command.correlation_id = EntityId::new(format!("corr_{idempotency_key}")).unwrap();
    command.causation_id = EntityId::new(format!("cause_{idempotency_key}")).unwrap();
    command
}
