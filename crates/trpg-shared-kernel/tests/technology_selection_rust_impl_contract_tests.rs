use trpg_shared_kernel::shared_kernel::{ActorRole, AuthorityMode, EventStore, TrpgError};
use trpg_shared_kernel::technology_selection_rust_impl::{
    append_technology_selection_rust_impl_reviewed, current_rust_technology_decisions,
    technology_selection_rust_impl_review, technology_selection_rust_landing,
    validate_technology_selection_rust_landing,
};

#[test]
fn technology_selection_rust_impl_accepts_current_safe_stack_choices() {
    let landing = technology_selection_rust_landing(current_rust_technology_decisions());

    validate_technology_selection_rust_landing(&landing).unwrap();
    assert_eq!(
        landing.governance_contract.module_name,
        "technology_selection_rust_impl"
    );
}

#[test]
fn technology_selection_rust_impl_rejects_empty_selection() {
    let mut decisions = current_rust_technology_decisions();
    for decision in &mut decisions {
        decision.selected = false;
    }
    let landing = technology_selection_rust_landing(decisions);

    assert!(matches!(
        validate_technology_selection_rust_landing(&landing),
        Err(TrpgError::WorkspaceViolation(_))
    ));
}

#[test]
fn technology_selection_rust_impl_rejects_direct_model_provider_access() {
    let mut decisions = current_rust_technology_decisions();
    decisions[0].direct_model_provider_access = true;
    let landing = technology_selection_rust_landing(decisions);

    assert_eq!(
        validate_technology_selection_rust_landing(&landing).unwrap_err(),
        TrpgError::PolicyDenied
    );
}

#[test]
fn technology_selection_rust_impl_review_is_recorded_as_a_governed_event() {
    let command = trpg_test_support::governed_command(
        technology_selection_rust_impl_review(),
        ActorRole::HumanKeeper,
        AuthorityMode::HumanKp,
    );
    let mut store = EventStore::default();

    let event = append_technology_selection_rust_impl_reviewed(&mut store, &command).unwrap();

    assert_eq!(event.payload.module_name, "technology_selection_rust_impl");
    assert_eq!(event.payload.reviewed_requirements, 3);
    assert_eq!(event.command_id, command.command_id);
}
