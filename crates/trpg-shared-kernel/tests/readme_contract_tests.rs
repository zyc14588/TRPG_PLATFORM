use trpg_shared_kernel::readme::{
    append_readme_reviewed, current_readme_contract, readme_contract, readme_review,
    validate_readme_contract,
};
use trpg_shared_kernel::shared_kernel::{ActorRole, AuthorityMode, EventStore, TrpgError};
use trpg_shared_kernel::workspace_and_governance::{
    validate_governance_contract, GovernanceSurface,
};

#[test]
fn readme_contract_points_to_current_governance_entry_points() {
    let contract = readme_contract();
    let readme = current_readme_contract();

    validate_governance_contract(&contract).unwrap();
    validate_readme_contract(&readme).unwrap();
    assert_eq!(contract.surface, GovernanceSurface::Readme);
    assert!(readme.points_to_top_level_design);
    assert!(readme.points_to_bootstrap_prompt);
    assert!(readme.points_to_normalized_maps);
    assert!(readme.states_historical_inputs_are_provenance_only);
    assert!(readme.forbids_direct_model_provider_access);
}

#[test]
fn readme_contract_rejects_missing_current_governance_links() {
    let mut readme = current_readme_contract();
    readme.points_to_top_level_design = false;

    assert!(matches!(
        validate_readme_contract(&readme),
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
    assert_eq!(event.payload.reviewed_requirements, 3);
    assert_eq!(event.authority_contract_version, 1);
}
