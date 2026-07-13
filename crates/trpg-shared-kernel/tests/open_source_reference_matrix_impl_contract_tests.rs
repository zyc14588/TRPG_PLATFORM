use trpg_shared_kernel::open_source_reference_matrix::{
    LicensePolicy, ReferenceEntry, ReferenceUse,
};
use trpg_shared_kernel::open_source_reference_matrix_impl::{
    append_open_source_reference_matrix_impl_reviewed, open_source_reference_matrix_impl_review,
    open_source_reference_matrix_landing, validate_open_source_reference_matrix_landing,
};
use trpg_shared_kernel::shared_kernel::{ActorRole, AuthorityMode, EventStore, TrpgError};

fn reviewed_reference() -> ReferenceEntry {
    ReferenceEntry {
        name: "rust".to_owned(),
        provenance_url: "https://www.rust-lang.org/".to_owned(),
        license_policy: LicensePolicy::Permissive,
        intended_use: ReferenceUse::DevelopmentTool,
        reviewed: true,
    }
}

#[test]
fn open_source_reference_matrix_impl_accepts_reviewed_references() {
    let landing = open_source_reference_matrix_landing(vec![reviewed_reference()]);

    validate_open_source_reference_matrix_landing(&landing).unwrap();
    assert_eq!(
        landing.governance_contract.module_name,
        "open_source_reference_matrix_impl"
    );
}

#[test]
fn open_source_reference_matrix_impl_rejects_unreviewed_non_permissive_reference() {
    let mut reference = reviewed_reference();
    reference.license_policy = LicensePolicy::ReciprocalReviewRequired;
    reference.reviewed = false;
    let landing = open_source_reference_matrix_landing(vec![reference]);

    assert!(matches!(
        validate_open_source_reference_matrix_landing(&landing),
        Err(TrpgError::OpenSourceReferenceViolation(_))
    ));
}

#[test]
fn open_source_reference_matrix_impl_rejects_empty_matrix() {
    let landing = open_source_reference_matrix_landing(Vec::new());

    assert!(matches!(
        validate_open_source_reference_matrix_landing(&landing),
        Err(TrpgError::OpenSourceReferenceViolation(_))
    ));
}

#[test]
fn open_source_reference_matrix_impl_review_is_recorded_as_a_governed_event() {
    let command = trpg_test_support::governed_command!(
        open_source_reference_matrix_impl_review(),
        ActorRole::HumanKeeper,
        AuthorityMode::HumanKp,
    );
    let mut store = EventStore::default();

    let event = append_open_source_reference_matrix_impl_reviewed(&mut store, &command).unwrap();

    assert_eq!(
        event.payload.module_name,
        "open_source_reference_matrix_impl"
    );
    assert_eq!(event.payload.reviewed_requirements, 3);
    assert_eq!(event.causation_id, command.causation_id);
}
