use trpg_shared_kernel::shared_kernel::{
    ActorRole, AuthorityMode, EventStore, FormalWritePath, TrpgError,
};
use trpg_shared_kernel::technology_selection_rust::{
    append_technology_selection_rust_reviewed, current_rust_technology_selections,
    technology_selection_rust_record, technology_selection_rust_review,
    validate_technology_selection_rust_record, TechnologySelectionRole,
};
use trpg_shared_kernel::workspace_and_governance::{
    validate_governance_contract, CanonicalStateBoundary, GovernanceSurface,
};

#[test]
fn technology_selection_rust_contract_uses_current_safe_outputs() {
    let record = technology_selection_rust_record(current_rust_technology_selections());
    let contract = &record.governance_contract;

    validate_technology_selection_rust_record(&record).unwrap();
    validate_governance_contract(contract).unwrap();
    assert_eq!(contract.module_name, "technology_selection_rust");
    assert_eq!(contract.surface, GovernanceSurface::TechnologySelectionRust);
    assert_eq!(
        contract.source_file,
        "crates/trpg-shared-kernel/src/technology_selection_rust.rs"
    );
    assert_eq!(
        contract.test_file,
        "crates/trpg-shared-kernel/tests/technology_selection_rust_contract_tests.rs"
    );
    assert_eq!(
        contract.canonical_state_boundary,
        CanonicalStateBoundary::EventStore
    );
    assert!(contract.requires_agent_gateway);
    assert!(!contract.permits_direct_model_provider_access);
}

#[test]
fn technology_selection_rust_rejects_missing_policy_or_direct_model_access() {
    let mut decisions = current_rust_technology_selections();
    decisions.retain(|decision| decision.role != TechnologySelectionRole::Policy);
    let record = technology_selection_rust_record(decisions);

    assert!(matches!(
        validate_technology_selection_rust_record(&record),
        Err(TrpgError::WorkspaceViolation(_))
    ));

    let mut decisions = current_rust_technology_selections();
    decisions[0].direct_model_provider_access = true;
    let record = technology_selection_rust_record(decisions);
    assert_eq!(
        validate_technology_selection_rust_record(&record).unwrap_err(),
        TrpgError::PolicyDenied
    );
}

#[test]
fn technology_selection_rust_review_is_recorded_through_event_store() {
    let command = trpg_test_support::governed_command!(
        technology_selection_rust_review(),
        ActorRole::HumanKeeper,
        AuthorityMode::HumanKp,
    );
    let mut store = EventStore::default();

    let event = append_technology_selection_rust_reviewed(&mut store, &command).unwrap();

    assert_eq!(event.sequence, 1);
    assert_eq!(event.payload.module_name, "technology_selection_rust");
    assert_eq!(event.payload.reviewed_requirements, 4);
    assert_eq!(event.idempotency_key, command.idempotency_key);
    assert_eq!(event.correlation_id, command.correlation_id);
    assert_eq!(event.causation_id, command.causation_id);
    assert_eq!(event.fact_provenance, command.fact_provenance);
}

#[test]
fn technology_selection_rust_blocks_direct_state_write_paths() {
    let mut command = trpg_test_support::governed_command!(
        technology_selection_rust_review(),
        ActorRole::System,
        AuthorityMode::AiKp,
    );
    command.write_path = FormalWritePath::DirectAgent;
    let mut store = EventStore::default();

    assert_eq!(
        append_technology_selection_rust_reviewed(&mut store, &command).unwrap_err(),
        TrpgError::DirectAgentStateWrite
    );
    assert!(store.events().is_empty());

    command.write_path = FormalWritePath::DirectBusiness;
    assert_eq!(
        append_technology_selection_rust_reviewed(&mut store, &command).unwrap_err(),
        TrpgError::PolicyDenied
    );
    assert!(store.events().is_empty());
}
