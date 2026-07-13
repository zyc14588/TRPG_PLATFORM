use trpg_shared_kernel::shared_kernel::{
    ActorRole, AuthorityMode, EventStore, FormalWritePath, TrpgError,
};
use trpg_shared_kernel::workspace_and_governance::{
    append_governance_reviewed, validate_governance_contract, workspace_governance_contract,
    workspace_governance_review, CanonicalStateBoundary, GovernanceSurface,
    REQUIRED_COMMAND_FIELDS,
};

#[test]
fn workspace_and_governance_contract_uses_current_safe_outputs() {
    let contract = workspace_governance_contract();

    validate_governance_contract(&contract).unwrap();
    assert_eq!(contract.module_name, "workspace_and_governance");
    assert_eq!(contract.surface, GovernanceSurface::WorkspaceAndGovernance);
    assert_eq!(
        contract.source_file,
        "crates/trpg-shared-kernel/src/workspace_and_governance.rs"
    );
    assert_eq!(
        contract.test_file,
        "crates/trpg-shared-kernel/tests/workspace_and_governance_contract_tests.rs"
    );
    assert_eq!(contract.command_fields, REQUIRED_COMMAND_FIELDS);
    assert_eq!(
        contract.canonical_state_boundary,
        CanonicalStateBoundary::EventStore
    );
}

#[test]
fn workspace_and_governance_rejects_governance_bypasses() {
    let mut direct_provider = workspace_governance_contract();
    direct_provider.permits_direct_model_provider_access = true;
    assert_eq!(
        validate_governance_contract(&direct_provider).unwrap_err(),
        TrpgError::PolicyDenied
    );

    let mut direct_agent = workspace_governance_contract();
    direct_agent.permits_direct_agent_state_write = true;
    assert_eq!(
        validate_governance_contract(&direct_agent).unwrap_err(),
        TrpgError::DirectAgentStateWrite
    );

    let mut mutable_authority = workspace_governance_contract();
    mutable_authority.permits_authority_contract_mutation = true;
    assert_eq!(
        validate_governance_contract(&mutable_authority).unwrap_err(),
        TrpgError::AuthorityContractMutation
    );
}

#[test]
fn workspace_and_governance_appends_reviews_through_event_store() {
    let command = trpg_test_support::governed_command!(
        workspace_governance_review(),
        ActorRole::HumanKeeper,
        AuthorityMode::HumanKp,
    );
    let mut store = EventStore::default();

    let event = append_governance_reviewed(&mut store, &command).unwrap();

    assert_eq!(event.sequence, 1);
    assert_eq!(event.command_id, command.command_id);
    assert_eq!(event.idempotency_key, command.idempotency_key);
    assert_eq!(event.correlation_id, command.correlation_id);
    assert_eq!(event.causation_id, command.causation_id);
    assert_eq!(event.visibility, command.visibility);
    assert_eq!(event.fact_provenance, command.fact_provenance);
    assert_eq!(event.payload.module_name, "workspace_and_governance");
    assert_eq!(store.events().len(), 1);

    let mut bypass = trpg_test_support::governed_command!(
        workspace_governance_review(),
        ActorRole::System,
        AuthorityMode::AiKp,
    );
    bypass.write_path = FormalWritePath::DirectAgent;

    assert_eq!(
        append_governance_reviewed(&mut store, &bypass).unwrap_err(),
        TrpgError::DirectAgentStateWrite
    );
    assert_eq!(store.events().len(), 1);
}
