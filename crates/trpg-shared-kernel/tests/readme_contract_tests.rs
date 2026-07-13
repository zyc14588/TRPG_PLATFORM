use trpg_shared_kernel::readme::{
    append_readme_reviewed, current_product_entry_boundary, readme_contract, readme_review,
    validate_product_entry_boundary,
};
use trpg_shared_kernel::shared_kernel::{ActorRole, AuthorityMode, EventStore, TrpgError};
use trpg_shared_kernel::workspace_and_governance::{
    validate_governance_contract, GovernanceSurface,
};

#[test]
fn readme_contract_preserves_product_entry_boundaries() {
    let contract = readme_contract();
    let boundary = current_product_entry_boundary();

    validate_governance_contract(&contract).unwrap();
    validate_product_entry_boundary(&boundary).unwrap();
    assert_eq!(contract.surface, GovernanceSurface::Readme);
    assert!(boundary.agent_gateway_required);
    assert!(boundary.direct_model_provider_access_forbidden);
    assert!(boundary.event_store_is_canonical);
    assert!(boundary.visibility_and_provenance_required);
}

#[test]
fn readme_contract_rejects_weakened_product_boundary() {
    let mut boundary = current_product_entry_boundary();
    boundary.event_store_is_canonical = false;

    assert!(matches!(
        validate_product_entry_boundary(&boundary),
        Err(TrpgError::WorkspaceViolation(_))
    ));
}

#[test]
fn readme_review_is_recorded_as_a_governed_event() {
    let command = trpg_test_support::governed_command!(
        readme_review(),
        ActorRole::HumanKeeper,
        AuthorityMode::HumanKp,
    );
    let mut store = EventStore::default();

    let event = append_readme_reviewed(&mut store, &command).unwrap();

    assert_eq!(event.payload.module_name, "readme");
    assert_eq!(event.payload.reviewed_requirements, 4);
    assert_eq!(event.authority_contract_version, 1);
}
