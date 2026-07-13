use trpg_shared_kernel::document_set::{current_foundation_document_set, FoundationDocument};
use trpg_shared_kernel::document_set_impl::{
    append_document_set_impl_reviewed, document_set_impl_review, document_set_landing,
    validate_document_set_landing,
};
use trpg_shared_kernel::shared_kernel::{ActorRole, AuthorityMode, EventStore, TrpgError};

#[test]
fn document_set_impl_landing_accepts_current_document_set() {
    let landing = document_set_landing(current_foundation_document_set());

    validate_document_set_landing(&landing).unwrap();
    assert_eq!(landing.governance_contract.module_name, "document_set_impl");
}

#[test]
fn document_set_impl_landing_rejects_incomplete_document_set() {
    let mut document_set = current_foundation_document_set();
    document_set
        .documents
        .retain(|document| *document != FoundationDocument::SafeOutputMap);
    let landing = document_set_landing(document_set);

    assert!(matches!(
        validate_document_set_landing(&landing),
        Err(TrpgError::WorkspaceViolation(_))
    ));
}

#[test]
fn document_set_impl_review_is_recorded_as_a_governed_event() {
    let command = trpg_test_support::governed_command!(
        document_set_impl_review(),
        ActorRole::HumanKeeper,
        AuthorityMode::HumanKp,
    );
    let mut store = EventStore::default();

    let event = append_document_set_impl_reviewed(&mut store, &command).unwrap();

    assert_eq!(event.payload.module_name, "document_set_impl");
    assert_eq!(event.payload.reviewed_requirements, 2);
    assert_eq!(event.fact_provenance, command.fact_provenance);
}
