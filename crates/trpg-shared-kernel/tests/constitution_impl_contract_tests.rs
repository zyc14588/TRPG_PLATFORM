use trpg_shared_kernel::constitution::{current_constitution_checklist, ConstitutionArticle};
use trpg_shared_kernel::constitution_impl::{
    append_constitution_impl_reviewed, constitution_impl_review, constitution_landing,
    validate_constitution_landing,
};
use trpg_shared_kernel::shared_kernel::{
    ActorRole, AuthorityMode, CommandEnvelope, EventStore, TrpgError,
};

#[test]
fn constitution_impl_landing_accepts_current_constitution() {
    let landing = constitution_landing(current_constitution_checklist());

    validate_constitution_landing(&landing).unwrap();
    assert_eq!(landing.governance_contract.module_name, "constitution_impl");
}

#[test]
fn constitution_impl_landing_rejects_incomplete_constitution() {
    let mut checklist = current_constitution_checklist();
    checklist
        .articles
        .retain(|article| *article != ConstitutionArticle::ServerDiceOnly);
    let landing = constitution_landing(checklist);

    assert!(matches!(
        validate_constitution_landing(&landing),
        Err(TrpgError::WorkspaceViolation(_))
    ));
}

#[test]
fn constitution_impl_review_is_recorded_as_a_governed_event() {
    let command = CommandEnvelope::governed(
        constitution_impl_review(),
        ActorRole::HumanKeeper,
        AuthorityMode::HumanKp,
    );
    let mut store = EventStore::default();

    let event = append_constitution_impl_reviewed(&mut store, &command).unwrap();

    assert_eq!(event.payload.module_name, "constitution_impl");
    assert_eq!(event.payload.reviewed_requirements, 2);
    assert_eq!(event.visibility, command.visibility);
}
