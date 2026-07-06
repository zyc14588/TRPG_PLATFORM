use std::collections::HashSet;

use trpg_data_eventing::{
    api_websocket_nats_contracts, batch_026_data_event_contracts, cache_redis_impl,
    domain_event_sourcing_projection, event_bus_nats_impl, is_current_safe_name,
    nats_subject_contracts, nats_subjects, nats_subjects_source_contract,
    persistence_postgresql_impl, rag_snapshot, rebuild_projection_from_events,
    replay_visible_data_events, ActorRole, AuthorityContract, AuthorityMode, CommandEnvelope,
    DataEventOperation, DataEventPayload, EventStore, FactProvenance, FormalWritePath,
    OutboxMessage, PrincipalScope, ProvenanceKind, TrpgError, Visibility, VisibilityLabel,
    COMMAND_ENVELOPE_REQUIRED_FIELDS, EVENT_ENVELOPE_REQUIRED_FIELDS, EVENT_STORE_TABLE,
    NATS_EVENTS_APPENDED, NATS_PROJECTION_REBUILD_REQUESTED, OUTBOX_TABLE,
};

const API_WS_NATS_FIXTURE: &str =
    include_str!("../../../fixtures/api/api_ws_nats_contract_cases.v1.json.md");
const RAG_SNAPSHOT_FIXTURE: &str =
    include_str!("../../../fixtures/rag/rag_snapshot_cases.v1.json.md");
const S03_DETAILED_FIXTURE: &str = include_str!(
    "../../../fixtures/stages/detailed/S03_event_store_projection_hash.current.json.md"
);

#[test]
fn b026_primary_contracts_map_to_current_safe_outputs() {
    let contracts = batch_026_data_event_contracts();
    assert_eq!(contracts.len(), 9);

    let expected = [
        (
            "CODEX-0626-06-DATA-EVENTING-59249231e5",
            "api_websocket_nats_contracts",
            DataEventOperation::SchemaRegister,
        ),
        (
            "CODEX-0627-06-DATA-EVENTING-e54d49d1d8",
            "nats_subjects",
            DataEventOperation::SchemaRegister,
        ),
        (
            "CODEX-0628-06-DATA-EVENTING-14e037cb2a",
            "nats_subject_contracts",
            DataEventOperation::SchemaRegister,
        ),
        (
            "CODEX-0630-06-DATA-EVENTING-fe8798d507",
            "nats_subjects_source_contract",
            DataEventOperation::SchemaRegister,
        ),
        (
            "CODEX-0634-06-DATA-EVENTING-12eb9d50b4",
            "domain_event_sourcing_projection",
            DataEventOperation::ProjectionRebuild,
        ),
        (
            "CODEX-0635-06-DATA-EVENTING-e414d1cc2e",
            "rag_snapshot",
            DataEventOperation::SnapshotCreate,
        ),
        (
            "CODEX-0636-06-DATA-EVENTING-2e55e84997",
            "cache_redis_impl",
            DataEventOperation::CacheWrite,
        ),
        (
            "CODEX-0637-06-DATA-EVENTING-745a12af17",
            "event_bus_nats_impl",
            DataEventOperation::OutboxPublish,
        ),
        (
            "CODEX-0638-06-DATA-EVENTING-0f91c8671e",
            "persistence_postgresql_impl",
            DataEventOperation::EventStoreAppend,
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
            .expect("B026 primary prompt has a contract");

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
fn b026_contract_metadata_covers_api_nats_rag_cache_and_persistence() {
    for required in [
        "idempotency_key",
        "expected_version",
        "visibility",
        "fact_provenance",
        "correlation_id",
        "causation_id",
    ] {
        assert!(api_websocket_nats_contracts::API_CONTRACT_REQUIRED_FIELDS.contains(&required));
        assert!(COMMAND_ENVELOPE_REQUIRED_FIELDS.contains(&required));
    }

    for required in ["visibility", "fact_provenance"] {
        assert!(EVENT_ENVELOPE_REQUIRED_FIELDS.contains(&required));
        assert!(nats_subject_contracts::SUBJECT_CONTRACT_METADATA.contains(&required));
        assert!(nats_subjects_source_contract::SOURCE_CONTRACT_FIELDS.contains(&required));
        assert!(rag_snapshot::RAG_SNAPSHOT_METADATA_FIELDS.contains(&required));
    }

    for subject in nats_subjects::REQUIRED_SUBJECTS {
        assert!(is_current_safe_name(subject));
    }
    assert!(nats_subjects::REQUIRED_SUBJECTS.contains(&NATS_EVENTS_APPENDED));
    assert!(nats_subjects::REQUIRED_SUBJECTS.contains(&NATS_PROJECTION_REBUILD_REQUESTED));

    for field in [
        "source_type",
        "version",
        "owner",
        "allowed_use",
        "chunk_hash",
    ] {
        assert!(rag_snapshot::RAG_SNAPSHOT_METADATA_FIELDS.contains(&field));
    }
    assert_eq!(rag_snapshot::RAG_REBUILD_SOURCE, EVENT_STORE_TABLE);
    assert_eq!(
        domain_event_sourcing_projection::PROJECTION_REBUILD_SOURCE,
        EVENT_STORE_TABLE
    );
    assert_eq!(cache_redis_impl::CACHE_REBUILD_SOURCE, EVENT_STORE_TABLE);
    let cache_is_canonical = std::hint::black_box(cache_redis_impl::CACHE_IS_CANONICAL);
    assert!(!cache_is_canonical);
    assert_eq!(event_bus_nats_impl::PUBLISH_SOURCE, OUTBOX_TABLE);
    assert!(event_bus_nats_impl::OUTBOX_FLOW_STATES.contains(&"dead_lettered"));
    assert!(persistence_postgresql_impl::TRANSACTIONAL_TABLES.contains(&EVENT_STORE_TABLE));
    assert!(persistence_postgresql_impl::TRANSACTIONAL_TABLES.contains(&OUTBOX_TABLE));
}

#[test]
fn b026_primary_surfaces_append_governed_events_and_bind_fixtures() {
    let authority_contract = authority_contract(AuthorityMode::AiKp);
    let mut store: EventStore<DataEventPayload> = EventStore::default();

    api_websocket_nats_contracts::append_api_websocket_nats_contracts_event(
        &mut store,
        &authority_contract,
        &governed_command(
            api_websocket_nats_contracts::ApiWebsocketNatsContractsCommand::record("api ws nats"),
            0,
            "idem_b026_surface_api",
            ActorRole::Workflow,
            AuthorityMode::AiKp,
        ),
    )
    .unwrap();
    nats_subjects::append_nats_subjects_event(
        &mut store,
        &authority_contract,
        &governed_command(
            nats_subjects::NatsSubjectsCommand::record("nats subjects"),
            1,
            "idem_b026_surface_subjects",
            ActorRole::Workflow,
            AuthorityMode::AiKp,
        ),
    )
    .unwrap();
    nats_subject_contracts::append_nats_subject_contracts_event(
        &mut store,
        &authority_contract,
        &governed_command(
            nats_subject_contracts::NatsSubjectContractsCommand::record("nats contract"),
            2,
            "idem_b026_surface_contract",
            ActorRole::Workflow,
            AuthorityMode::AiKp,
        ),
    )
    .unwrap();
    nats_subjects_source_contract::append_nats_subjects_source_contract_event(
        &mut store,
        &authority_contract,
        &governed_command(
            nats_subjects_source_contract::NatsSubjectsSourceContractCommand::record(
                "source contract",
            ),
            3,
            "idem_b026_surface_source",
            ActorRole::Workflow,
            AuthorityMode::AiKp,
        ),
    )
    .unwrap();
    domain_event_sourcing_projection::append_domain_event_sourcing_projection_event(
        &mut store,
        &authority_contract,
        &governed_command(
            domain_event_sourcing_projection::DomainEventSourcingProjectionCommand::record(
                "projection",
            ),
            4,
            "idem_b026_surface_projection",
            ActorRole::Workflow,
            AuthorityMode::AiKp,
        ),
    )
    .unwrap();
    rag_snapshot::append_rag_snapshot_event(
        &mut store,
        &authority_contract,
        &governed_command(
            rag_snapshot::RagSnapshotCommand::record("rag snapshot"),
            5,
            "idem_b026_surface_rag",
            ActorRole::Workflow,
            AuthorityMode::AiKp,
        ),
    )
    .unwrap();
    cache_redis_impl::append_cache_redis_impl_event(
        &mut store,
        &authority_contract,
        &governed_command(
            cache_redis_impl::CacheRedisImplCommand::record("cache"),
            6,
            "idem_b026_surface_cache",
            ActorRole::Workflow,
            AuthorityMode::AiKp,
        ),
    )
    .unwrap();
    let event_bus_event = event_bus_nats_impl::append_event_bus_nats_impl_event(
        &mut store,
        &authority_contract,
        &governed_command(
            event_bus_nats_impl::EventBusNatsImplCommand::record("outbox publish"),
            7,
            "idem_b026_surface_bus",
            ActorRole::Workflow,
            AuthorityMode::AiKp,
        ),
    )
    .unwrap();
    persistence_postgresql_impl::append_persistence_postgresql_impl_event(
        &mut store,
        &authority_contract,
        &governed_command(
            persistence_postgresql_impl::PersistencePostgresqlImplCommand::record("persistence"),
            8,
            "idem_b026_surface_persistence",
            ActorRole::Workflow,
            AuthorityMode::AiKp,
        ),
    )
    .unwrap();

    assert_eq!(store.events().len(), 9);
    let modules: HashSet<_> = store
        .events()
        .iter()
        .map(|event| event.payload.module_name)
        .collect();
    for contract in batch_026_data_event_contracts() {
        assert!(modules.contains(contract.module_name));
    }

    let outbox = OutboxMessage::from(&event_bus_event);
    assert_eq!(outbox.event_id, 8);
    assert_eq!(
        event_bus_event.payload.operation,
        DataEventOperation::OutboxPublish
    );
    assert!(event_bus_event.payload.read_models.contains(&OUTBOX_TABLE));

    let projection = rebuild_projection_from_events(store.events());
    assert_eq!(projection.event_count, 9);
    assert_eq!(projection.last_sequence, 9);
    assert_eq!(rag_snapshot::RAG_REBUILD_SOURCE, EVENT_STORE_TABLE);
    assert_eq!(cache_redis_impl::CACHE_REBUILD_SOURCE, EVENT_STORE_TABLE);
    assert!(persistence_postgresql_impl::TRANSACTIONAL_TABLES.contains(&OUTBOX_TABLE));

    for required_fixture_token in [
        "\"private_to_player:user_player_a\"",
        "\"trpg.llm.direct.*\"",
        "\"keeper_truth_not_in_player_rag\"",
        "\"expected_player_context\": \"REDACTED\"",
        "\"OutboxMessage\"",
        "\"projection_hash_stable\"",
    ] {
        assert!(
            API_WS_NATS_FIXTURE.contains(required_fixture_token)
                || RAG_SNAPSHOT_FIXTURE.contains(required_fixture_token)
                || S03_DETAILED_FIXTURE.contains(required_fixture_token),
            "missing fixture token: {required_fixture_token}"
        );
    }
}

#[test]
fn b026_appends_only_through_governed_event_store_path() {
    let authority_contract = authority_contract(AuthorityMode::AiKp);
    let mut store: EventStore<DataEventPayload> = EventStore::default();

    let first = governed_command(
        api_websocket_nats_contracts::ApiWebsocketNatsContractsCommand::record("api contract"),
        0,
        "idem_b026_api",
        ActorRole::Workflow,
        AuthorityMode::AiKp,
    );
    let event = api_websocket_nats_contracts::append_api_websocket_nats_contracts_event(
        &mut store,
        &authority_contract,
        &first,
    )
    .unwrap();

    assert_eq!(event.sequence, 1);
    assert_eq!(
        event.payload.module_name,
        api_websocket_nats_contracts::MODULE_NAME
    );
    assert_eq!(event.fact_provenance.reference.as_str(), "fact_b026");

    let stale_version = governed_command(
        nats_subjects::NatsSubjectsCommand::record("stale subject update"),
        0,
        "idem_b026_stale",
        ActorRole::Workflow,
        AuthorityMode::AiKp,
    );
    let error =
        nats_subjects::append_nats_subjects_event(&mut store, &authority_contract, &stale_version)
            .unwrap_err();
    assert_eq!(
        error,
        TrpgError::ExpectedVersionConflict {
            expected: 0,
            actual: 1,
        }
    );

    let duplicate = governed_command(
        nats_subject_contracts::NatsSubjectContractsCommand::record("duplicate"),
        1,
        "idem_b026_api",
        ActorRole::Workflow,
        AuthorityMode::AiKp,
    );
    let error = nats_subject_contracts::append_nats_subject_contracts_event(
        &mut store,
        &authority_contract,
        &duplicate,
    )
    .unwrap_err();
    assert_eq!(error.code(), "DUPLICATE_COMMAND");

    let mut direct_agent = governed_command(
        cache_redis_impl::CacheRedisImplCommand::record("agent cache bypass"),
        1,
        "idem_b026_direct_agent",
        ActorRole::Workflow,
        AuthorityMode::AiKp,
    );
    direct_agent.write_path = FormalWritePath::DirectAgent;
    let error = cache_redis_impl::append_cache_redis_impl_event(
        &mut store,
        &authority_contract,
        &direct_agent,
    )
    .unwrap_err();
    assert_eq!(error.code(), "DIRECT_AGENT_STATE_WRITE");

    let bad_actor = governed_command(
        event_bus_nats_impl::EventBusNatsImplCommand::record("human keeper in ai mode"),
        1,
        "idem_b026_bad_actor",
        ActorRole::HumanKeeper,
        AuthorityMode::AiKp,
    );
    let error = event_bus_nats_impl::append_event_bus_nats_impl_event(
        &mut store,
        &authority_contract,
        &bad_actor,
    )
    .unwrap_err();
    assert_eq!(error.code(), "AUTHORITY_VIOLATION");
    assert_eq!(store.events().len(), 1);
}

#[test]
fn b026_visibility_provenance_and_projection_replay_are_preserved() {
    let authority_contract = authority_contract(AuthorityMode::AiKp);
    let mut store: EventStore<DataEventPayload> = EventStore::default();
    let player_a = trpg_data_eventing::EntityId::new("player_b026_a").unwrap();
    let player_b = trpg_data_eventing::EntityId::new("player_b026_b").unwrap();

    let mut private_to_a = governed_command(
        domain_event_sourcing_projection::DomainEventSourcingProjectionCommand::record(
            "private projection",
        ),
        0,
        "idem_b026_private",
        ActorRole::Workflow,
        AuthorityMode::AiKp,
    );
    private_to_a.visibility = Visibility::private_to_player(player_a.clone());
    domain_event_sourcing_projection::append_domain_event_sourcing_projection_event(
        &mut store,
        &authority_contract,
        &private_to_a,
    )
    .unwrap();

    let mut keeper_only = governed_command(
        rag_snapshot::RagSnapshotCommand::record("keeper rag snapshot"),
        1,
        "idem_b026_keeper",
        ActorRole::Workflow,
        AuthorityMode::AiKp,
    );
    keeper_only.visibility = Visibility::new(VisibilityLabel::KeeperOnly);
    rag_snapshot::append_rag_snapshot_event(&mut store, &authority_contract, &keeper_only).unwrap();

    let mut ai_internal = governed_command(
        persistence_postgresql_impl::PersistencePostgresqlImplCommand::record("internal db trace"),
        2,
        "idem_b026_ai_internal",
        ActorRole::Workflow,
        AuthorityMode::AiKp,
    );
    ai_internal.visibility = Visibility::new(VisibilityLabel::AiInternal);
    persistence_postgresql_impl::append_persistence_postgresql_impl_event(
        &mut store,
        &authority_contract,
        &ai_internal,
    )
    .unwrap();

    let first = rebuild_projection_from_events(store.events());
    let second = rebuild_projection_from_events(store.events());
    assert_eq!(first, second);
    assert_eq!(first.event_count, 3);
    assert_eq!(first.last_sequence, 3);
    assert!(store
        .events()
        .iter()
        .all(|event| event.fact_provenance.reference.as_str() == "fact_b026"));

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

fn authority_contract(mode: AuthorityMode) -> AuthorityContract {
    AuthorityContract::new("campaign_batch_026", mode, 1).unwrap()
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
        FactProvenance::new(ProvenanceKind::SystemFixture, "fact_b026", "batch_026").unwrap();
    command
}
