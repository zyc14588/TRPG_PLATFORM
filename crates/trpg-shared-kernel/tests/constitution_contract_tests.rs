use trpg_shared_kernel::constitution::{
    append_constitution_reviewed, constitution_contract, constitution_review,
    current_constitution_checklist, validate_constitution_checklist, ConstitutionArticle,
};
use trpg_shared_kernel::shared_kernel::{ActorRole, AuthorityMode, EventStore, TrpgError};
use trpg_shared_kernel::workspace_and_governance::{
    validate_governance_contract, GovernanceSurface,
};

#[test]
fn constitution_contract_preserves_top_level_articles() {
    let contract = constitution_contract();
    let checklist = current_constitution_checklist();

    validate_governance_contract(&contract).unwrap();
    validate_constitution_checklist(&checklist).unwrap();
    assert_eq!(contract.surface, GovernanceSurface::Constitution);
    assert!(checklist
        .articles
        .contains(&ConstitutionArticle::AuthorityContractImmutable));
    assert!(checklist
        .articles
        .contains(&ConstitutionArticle::AgentGatewayRequired));
    assert!(checklist
        .articles
        .contains(&ConstitutionArticle::FormalWritesUseEventStore));
    assert!(checklist
        .articles
        .contains(&ConstitutionArticle::VisibilityAndProvenanceRequired));
    assert!(checklist
        .articles
        .contains(&ConstitutionArticle::ServerDiceOnly));
}

#[test]
fn constitution_contract_rejects_missing_articles() {
    let mut checklist = current_constitution_checklist();
    checklist
        .articles
        .retain(|article| *article != ConstitutionArticle::AuthorityContractImmutable);

    assert!(matches!(
        validate_constitution_checklist(&checklist),
        Err(TrpgError::WorkspaceViolation(_))
    ));
}

#[test]
fn constitution_review_is_recorded_as_a_governed_event() {
    let command = trpg_test_support::governed_command(
        constitution_review(),
        ActorRole::HumanKeeper,
        AuthorityMode::HumanKp,
    );
    let mut store = EventStore::default();

    let event = append_constitution_reviewed(&mut store, &command).unwrap();

    assert_eq!(event.sequence, 1);
    assert_eq!(event.payload.module_name, "constitution");
    assert_eq!(event.payload.reviewed_requirements, 5);
    assert_eq!(event.visibility, command.visibility);
    assert_eq!(event.fact_provenance, command.fact_provenance);
}
