use trpg_data_eventing::{
    all_data_event_contracts, batch_028_data_event_contracts, event_json_schema,
    is_current_safe_name, replay_visible_data_events, ActorRole, AuthorityContract, AuthorityMode,
    CommandEnvelope, DataEventOperation, DataEventPayload, EntityId, EventStore, FactProvenance,
    FormalWritePath, PrincipalScope, ProvenanceKind, TrpgError, Visibility, VisibilityLabel,
    COMMAND_ENVELOPE_REQUIRED_FIELDS, EVENT_ENVELOPE_REQUIRED_FIELDS, EVENT_STORE_TABLE,
    NATS_EVENTS_APPENDED, NATS_PROJECTION_REBUILD_REQUESTED, OUTBOX_TABLE,
};

const P0107_PROMPT: &str = include_str!("../../../codex-prompts/06-data-eventing/P0107.md");
const S03_DETAILED_FIXTURE: &str = include_str!(
    "../../../fixtures/stages/detailed/S03_event_store_projection_hash.current.json.md"
);
const EVENT_STREAM_FIXTURE: &str =
    include_str!("../../../fixtures/event_store/golden_event_stream_expected.v1.json.md");
const RAG_SNAPSHOT_FIXTURE: &str =
    include_str!("../../../fixtures/rag/rag_snapshot_cases.v1.json.md");

#[test]
fn event_json_schema_contract_maps_to_current_safe_primary_output() {
    let contracts = batch_028_data_event_contracts();
    assert_eq!(contracts.len(), 1);

    let contract = &contracts[0];
    assert_eq!(contract.prompt_id, "CODEX-0682-06-DATA-EVENTING-af0d5b5090");
    assert_eq!(contract.module_name, "event_json_schema");
    assert_eq!(contract.event_type, "EventJsonSchemaRegistered");
    assert_eq!(contract.operation, DataEventOperation::SchemaRegister);
    assert_eq!(contract.event_store_table, EVENT_STORE_TABLE);
    assert_eq!(contract.outbox_table, OUTBOX_TABLE);
    assert!(contract.nats_subjects.contains(&NATS_EVENTS_APPENDED));
    assert!(contract
        .nats_subjects
        .contains(&NATS_PROJECTION_REBUILD_REQUESTED));
    assert!(contract.uses_current_safe_names());

    let all_contracts = all_data_event_contracts();
    assert!(all_contracts
        .iter()
        .any(|candidate| candidate.prompt_id == contract.prompt_id));
    for name in [
        contract.module_name,
        contract.event_type,
        contract.event_schema_name,
        contract.projection_name,
    ] {
        assert!(is_current_safe_name(name));
    }
}

#[test]
fn event_json_schema_catalog_declares_governed_command_and_event_fields() {
    let entry = event_json_schema::catalog_entry();

    assert_eq!(entry.schema_name, event_json_schema::EVENT_SCHEMA_NAME);
    assert_eq!(entry.schema_kind, "event_command_schema_aggregate");
    assert_eq!(
        entry.canonical_write_flow,
        "command_workflow_decision_event_store_projection"
    );
    assert_eq!(entry.read_model_policy, "projection_cache_rag_rebuildable");
    assert_eq!(
        event_json_schema::command_schema_fields(),
        COMMAND_ENVELOPE_REQUIRED_FIELDS
    );
    assert_eq!(
        event_json_schema::event_schema_fields(),
        EVENT_ENVELOPE_REQUIRED_FIELDS
    );

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
        assert!(entry.command_fields.contains(&required));
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
        assert!(entry.event_fields.contains(&required));
    }

    for required in [
        "public",
        "party_visible",
        "private_to_player",
        "keeper_only",
        "ai_internal",
        "system_only",
    ] {
        assert!(entry.visibility_labels.contains(&required));
    }

    for required in [
        "AUTHORITY_VIOLATION",
        "AUTHORITY_CONTRACT_MUTATION",
        "DIRECT_AGENT_STATE_WRITE",
        "POLICY_DENIED",
        "EXPECTED_VERSION_CONFLICT",
        "DUPLICATE_COMMAND",
    ] {
        assert!(entry.governance_error_codes.contains(&required));
    }
}

#[test]
fn event_json_schema_appends_only_through_governed_event_store_path() {
    let authority_contract = authority_contract(AuthorityMode::AiKp);
    let mut store: EventStore<DataEventPayload> = EventStore::default();

    let first = governed_command(
        0,
        "idem_b028_schema_001",
        Visibility::new(VisibilityLabel::SystemOnly),
    );
    let event =
        event_json_schema::append_event_json_schema_event(&mut store, &authority_contract, &first)
            .unwrap();

    assert_eq!(event.sequence, 1);
    assert_eq!(event.payload.module_name, event_json_schema::MODULE_NAME);
    assert_eq!(event.payload.event_name, event_json_schema::EVENT_TYPE);
    assert_eq!(event.payload.operation, DataEventOperation::SchemaRegister);
    assert!(event.payload.read_models.contains(&EVENT_STORE_TABLE));
    assert!(event.payload.read_models.contains(&OUTBOX_TABLE));
    assert_eq!(event.correlation_id.as_str(), "corr_idem_b028_schema_001");
    assert_eq!(event.causation_id.as_str(), "cause_idem_b028_schema_001");

    let stale = governed_command(
        0,
        "idem_b028_schema_stale",
        Visibility::new(VisibilityLabel::SystemOnly),
    );
    let error =
        event_json_schema::append_event_json_schema_event(&mut store, &authority_contract, &stale)
            .unwrap_err();
    assert_eq!(
        error,
        TrpgError::ExpectedVersionConflict {
            expected: 0,
            actual: 1,
        }
    );

    let duplicate = governed_command(
        1,
        "idem_b028_schema_001",
        Visibility::new(VisibilityLabel::SystemOnly),
    );
    let error = event_json_schema::append_event_json_schema_event(
        &mut store,
        &authority_contract,
        &duplicate,
    )
    .unwrap_err();
    assert_eq!(error.code(), "DUPLICATE_COMMAND");

    let bad_actor = governed_command_with_role(
        1,
        "idem_b028_schema_bad_actor",
        ActorRole::HumanKeeper,
        Visibility::new(VisibilityLabel::SystemOnly),
    );
    let error = event_json_schema::append_event_json_schema_event(
        &mut store,
        &authority_contract,
        &bad_actor,
    )
    .unwrap_err();
    assert_eq!(error.code(), "AUTHORITY_VIOLATION");

    let mut direct_agent = governed_command(
        1,
        "idem_b028_schema_direct_agent",
        Visibility::new(VisibilityLabel::SystemOnly),
    );
    direct_agent.write_path = FormalWritePath::DirectAgent;
    let error = event_json_schema::append_event_json_schema_event(
        &mut store,
        &authority_contract,
        &direct_agent,
    )
    .unwrap_err();
    assert_eq!(error.code(), "DIRECT_AGENT_STATE_WRITE");
    assert_eq!(store.events().len(), 1);
}

#[test]
fn event_json_schema_preserves_visibility_provenance_and_fixture_bindings() {
    assert!(P0107_PROMPT.contains("CODEX-0682-06-DATA-EVENTING-af0d5b5090"));
    assert!(P0107_PROMPT.contains("data_eventing::event_json_schema"));
    assert!(S03_DETAILED_FIXTURE.contains("\"ProjectionRebuilt\""));
    assert!(EVENT_STREAM_FIXTURE.contains("\"idempotency_repeat\""));
    assert!(RAG_SNAPSHOT_FIXTURE.contains("\"expected_player_context\": \"REDACTED\""));

    let authority_contract = authority_contract(AuthorityMode::AiKp);
    let mut store: EventStore<DataEventPayload> = EventStore::default();
    let player_a = EntityId::new("player_b028_a").unwrap();
    let player_b = EntityId::new("player_b028_b").unwrap();

    let mut private_to_a = governed_command(
        0,
        "idem_b028_schema_private",
        Visibility::private_to_player(player_a.clone()),
    );
    private_to_a.fact_provenance =
        FactProvenance::new(ProvenanceKind::ToolResult, "fact_b028_private", "tool_b028").unwrap();
    event_json_schema::append_event_json_schema_event(
        &mut store,
        &authority_contract,
        &private_to_a,
    )
    .unwrap();

    event_json_schema::append_event_json_schema_event(
        &mut store,
        &authority_contract,
        &governed_command(
            1,
            "idem_b028_schema_keeper",
            Visibility::new(VisibilityLabel::KeeperOnly),
        ),
    )
    .unwrap();
    event_json_schema::append_event_json_schema_event(
        &mut store,
        &authority_contract,
        &governed_command(
            2,
            "idem_b028_schema_ai",
            Visibility::new(VisibilityLabel::AiInternal),
        ),
    )
    .unwrap();

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
    assert_eq!(
        store.events()[0].fact_provenance.reference.as_str(),
        "fact_b028_private"
    );
}

fn authority_contract(mode: AuthorityMode) -> AuthorityContract {
    AuthorityContract::new("campaign_batch_028", mode, 1).unwrap()
}

fn governed_command(
    expected_version: u64,
    idempotency_key: &'static str,
    visibility: Visibility,
) -> CommandEnvelope<event_json_schema::EventJsonSchemaCommand> {
    governed_command_with_role(
        expected_version,
        idempotency_key,
        ActorRole::Workflow,
        visibility,
    )
}

fn governed_command_with_role(
    expected_version: u64,
    idempotency_key: &'static str,
    role: ActorRole,
    visibility: Visibility,
) -> CommandEnvelope<event_json_schema::EventJsonSchemaCommand> {
    let mut command = CommandEnvelope::governed(
        event_json_schema::EventJsonSchemaCommand::record("schema aggregate"),
        role,
        AuthorityMode::AiKp,
    );
    command.command_id = EntityId::new(format!("command_{idempotency_key}")).unwrap();
    command.idempotency_key = idempotency_key.to_owned();
    command.expected_version = expected_version;
    command.visibility = visibility;
    command.fact_provenance =
        FactProvenance::new(ProvenanceKind::SystemFixture, "fact_b028", "batch_028").unwrap();
    command.correlation_id = EntityId::new(format!("corr_{idempotency_key}")).unwrap();
    command.causation_id = EntityId::new(format!("cause_{idempotency_key}")).unwrap();
    command
}
