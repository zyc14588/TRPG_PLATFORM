use trpg_shared_kernel::constitution::ConstitutionArticle;
use trpg_shared_kernel::document_set::{
    append_document_set_reviewed, current_governance_checklist, document_set_contract,
    document_set_review, validate_governance_checklist,
};
use trpg_shared_kernel::shared_kernel::{ActorRole, AuthorityMode, EventStore, TrpgError};
use trpg_shared_kernel::workspace_and_governance::{
    validate_governance_contract, GovernanceSurface,
};

#[test]
fn document_set_contract_requires_product_governance_invariants() {
    let contract = document_set_contract();
    let checklist = current_governance_checklist();

    validate_governance_contract(&contract).unwrap();
    validate_governance_checklist(&checklist).unwrap();
    assert_eq!(contract.surface, GovernanceSurface::DocumentSet);
    assert!(checklist
        .articles
        .contains(&ConstitutionArticle::AuthorityContractImmutable));
    assert!(checklist
        .articles
        .contains(&ConstitutionArticle::AgentGatewayRequired));
    assert!(checklist
        .articles
        .contains(&ConstitutionArticle::FormalWritesUseEventStore));
}

#[test]
fn document_set_contract_rejects_missing_product_invariant() {
    let mut missing = current_governance_checklist();
    missing
        .articles
        .retain(|article| *article != ConstitutionArticle::ServerDiceOnly);

    assert!(matches!(
        validate_governance_checklist(&missing),
        Err(TrpgError::WorkspaceViolation(_))
    ));
}

#[test]
fn document_set_review_is_recorded_as_a_governed_event() {
    let command = trpg_test_support::governed_command!(
        document_set_review(),
        ActorRole::HumanKeeper,
        AuthorityMode::HumanKp,
    );
    let mut store = EventStore::default();

    let event = append_document_set_reviewed(&mut store, &command).unwrap();

    assert_eq!(event.payload.module_name, "document_set");
    assert_eq!(event.payload.reviewed_requirements, 5);
    assert_eq!(event.correlation_id, command.correlation_id);
    assert_eq!(event.causation_id, command.causation_id);
}
