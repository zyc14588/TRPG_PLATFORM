use trpg_shared_kernel::document_set::{
    append_document_set_reviewed, current_foundation_document_set, document_set_contract,
    document_set_review, validate_foundation_document_set, FoundationDocument,
};
use trpg_shared_kernel::shared_kernel::{ActorRole, AuthorityMode, EventStore, TrpgError};
use trpg_shared_kernel::workspace_and_governance::{
    validate_governance_contract, GovernanceSurface,
};

#[test]
fn document_set_contract_requires_current_authority_inputs() {
    let contract = document_set_contract();
    let document_set = current_foundation_document_set();

    validate_governance_contract(&contract).unwrap();
    validate_foundation_document_set(&document_set).unwrap();
    assert_eq!(contract.surface, GovernanceSurface::DocumentSet);
    assert!(document_set
        .documents
        .contains(&FoundationDocument::TopLevelDesign));
    assert!(document_set
        .documents
        .contains(&FoundationDocument::NormalizedExecutionMap));
    assert!(document_set
        .documents
        .contains(&FoundationDocument::SafeOutputMap));
    assert!(document_set
        .documents
        .contains(&FoundationDocument::TokenRewriteTable));
}

#[test]
fn document_set_contract_rejects_missing_or_promoted_history() {
    let mut missing = current_foundation_document_set();
    missing
        .documents
        .retain(|document| *document != FoundationDocument::TokenRewriteTable);

    assert!(matches!(
        validate_foundation_document_set(&missing),
        Err(TrpgError::WorkspaceViolation(_))
    ));

    let mut promoted_history = current_foundation_document_set();
    promoted_history.historical_inputs_are_provenance_only = false;

    assert!(matches!(
        validate_foundation_document_set(&promoted_history),
        Err(TrpgError::WorkspaceViolation(_))
    ));
}

#[test]
fn document_set_review_is_recorded_as_a_governed_event() {
    let command = trpg_test_support::governed_command(
        document_set_review(),
        ActorRole::HumanKeeper,
        AuthorityMode::HumanKp,
    );
    let mut store = EventStore::default();

    let event = append_document_set_reviewed(&mut store, &command).unwrap();

    assert_eq!(event.payload.module_name, "document_set");
    assert_eq!(event.payload.reviewed_requirements, 3);
    assert_eq!(event.correlation_id, command.correlation_id);
    assert_eq!(event.causation_id, command.causation_id);
}
