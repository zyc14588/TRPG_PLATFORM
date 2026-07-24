use trpg_shared_kernel::adr_0001_rust_first::{
    adr_0001_rust_first_record, adr_0001_rust_first_review, append_adr_0001_rust_first_reviewed,
    current_rust_first_decisions, validate_adr_0001_rust_first_record,
};
use trpg_shared_kernel::shared_kernel::{
    ActorRole, AuthorityMode, EntityId, EventEnvelopeWire, EventStore, PrincipalScope, TrpgError,
    Visibility, EVENT_ENVELOPE_WIRE_SCHEMA_VERSION,
};
use trpg_shared_kernel::workspace_and_governance::{
    validate_governance_contract, CanonicalStateBoundary, GovernanceSurface,
};

#[test]
fn adr_0001_rust_first_contract_uses_current_safe_outputs() {
    let record = adr_0001_rust_first_record(current_rust_first_decisions());
    let contract = &record.governance_contract;

    validate_adr_0001_rust_first_record(&record).unwrap();
    validate_governance_contract(contract).unwrap();
    assert_eq!(contract.module_name, "adr_0001_rust_first");
    assert_eq!(contract.surface, GovernanceSurface::Adr0001RustFirst);
    assert_eq!(
        contract.source_file,
        "crates/trpg-shared-kernel/src/adr_0001_rust_first.rs"
    );
    assert_eq!(
        contract.test_file,
        "crates/trpg-shared-kernel/tests/adr_0001_rust_first_contract_tests.rs"
    );
    assert_eq!(
        contract.canonical_state_boundary,
        CanonicalStateBoundary::EventStore
    );
    assert!(contract.requires_agent_gateway);
    assert!(!contract.permits_direct_model_provider_access);
}

#[test]
fn adr_0001_rust_first_rejects_governance_bypasses() {
    let mut decisions = current_rust_first_decisions();
    decisions[0].bypasses_event_store = true;
    let record = adr_0001_rust_first_record(decisions);

    assert!(matches!(
        validate_adr_0001_rust_first_record(&record),
        Err(TrpgError::WorkspaceViolation(_))
    ));

    let mut decisions = current_rust_first_decisions();
    decisions[1].direct_model_provider_access = true;
    let record = adr_0001_rust_first_record(decisions);
    assert_eq!(
        validate_adr_0001_rust_first_record(&record).unwrap_err(),
        TrpgError::PolicyDenied
    );

    let mut decisions = current_rust_first_decisions();
    decisions[2].mutates_authority_contract = true;
    let record = adr_0001_rust_first_record(decisions);
    assert_eq!(
        validate_adr_0001_rust_first_record(&record).unwrap_err(),
        TrpgError::AuthorityContractMutation
    );
}

#[test]
fn adr_0001_rust_first_review_is_recorded_through_event_store() {
    let command = trpg_test_support::governed_command(
        adr_0001_rust_first_review(),
        ActorRole::HumanKeeper,
        AuthorityMode::HumanKp,
    );
    let mut store = EventStore::default();

    let event = append_adr_0001_rust_first_reviewed(&mut store, &command).unwrap();

    assert_eq!(event.sequence, 1);
    assert_eq!(event.command_id, command.command_id);
    assert_eq!(event.idempotency_key, command.idempotency_key);
    assert_eq!(event.correlation_id, command.correlation_id);
    assert_eq!(event.causation_id, command.causation_id);
    assert_eq!(event.visibility, command.visibility);
    assert_eq!(event.fact_provenance, command.fact_provenance);
    assert_eq!(event.payload.module_name, "adr_0001_rust_first");
    assert_eq!(event.payload.reviewed_requirements, 4);

    let canonical_json = event.to_canonical_json().unwrap();
    let wire: EventEnvelopeWire<serde_json::Value> = serde_json::from_str(&canonical_json).unwrap();
    assert_eq!(wire.schema_version, EVENT_ENVELOPE_WIRE_SCHEMA_VERSION);
    assert_eq!(wire.stream_id, event.stream_id.as_str());
    assert_eq!(wire.stream_version, event.stream_version);
    assert_eq!(wire.visibility_label, event.visibility.label().as_str());
    assert_eq!(
        wire.provenance_reference,
        event.fact_provenance.reference.as_str()
    );
    assert_eq!(wire.to_canonical_json().unwrap(), canonical_json);

    let duplicate = trpg_test_support::governed_command(
        adr_0001_rust_first_review(),
        ActorRole::HumanKeeper,
        AuthorityMode::HumanKp,
    );
    let retried = append_adr_0001_rust_first_reviewed(&mut store, &duplicate).unwrap();
    assert_eq!(retried, event);
    assert_eq!(store.events().len(), 1);
}

#[test]
fn adr_0001_rust_first_preserves_visibility_and_authority_guards() {
    let player = EntityId::new("character_001").unwrap();
    let mut command = trpg_test_support::governed_command(
        adr_0001_rust_first_review(),
        ActorRole::HumanKeeper,
        AuthorityMode::HumanKp,
    );
    command.visibility = Visibility::private_to_player(player.clone());

    let mut store = EventStore::default();
    append_adr_0001_rust_first_reviewed(&mut store, &command).unwrap();

    assert_eq!(
        store.replay_visible(&PrincipalScope::Player(player)).len(),
        1
    );
    assert!(store.replay_visible(&PrincipalScope::Public).is_empty());

    let invalid_authority = trpg_test_support::governed_command(
        adr_0001_rust_first_review(),
        ActorRole::AiKeeper,
        AuthorityMode::AiKp,
    );
    assert_eq!(
        append_adr_0001_rust_first_reviewed(&mut store, &invalid_authority).unwrap_err(),
        TrpgError::AuthorityViolation
    );
}

#[test]
fn p04_seeded_stream_version_is_scoped_to_one_campaign_stream() {
    let contract_a =
        trpg_test_support::authority_contract("campaign_p04_seeded_a", AuthorityMode::HumanKp, 1)
            .unwrap();
    let contract_b =
        trpg_test_support::authority_contract("campaign_p04_fresh_b", AuthorityMode::HumanKp, 1)
            .unwrap();
    let mut store: EventStore<String> = EventStore::with_stream_base_version_for(
        contract_a.campaign_id().clone(),
        contract_a.campaign_id().clone(),
        5,
    );

    let mut command_a = trpg_test_support::governed_command_for_contract(
        &contract_a,
        "seeded stream".to_owned(),
        ActorRole::HumanKeeper,
    );
    command_a.expected_version = 5;
    let event_a = store
        .append(&command_a, "P04SeededStreamRecorded", "seeded".to_owned())
        .unwrap();

    let command_b = trpg_test_support::governed_command_for_contract(
        &contract_b,
        "fresh stream".to_owned(),
        ActorRole::HumanKeeper,
    );
    let event_b = store
        .append(&command_b, "P04FreshStreamRecorded", "fresh".to_owned())
        .unwrap();

    assert_eq!(event_a.stream_version, 6);
    assert_eq!(event_b.stream_version, 1);
    assert_eq!(event_a.sequence, 1);
    assert_eq!(event_b.sequence, 2);
}
