use trpg_shared_kernel::config_model::LocalModelCertification;
use trpg_shared_kernel::open_source_reference_matrix::{
    validate_local_model_for_ai_keeper, validate_reference_entry, LicensePolicy, ReferenceEntry,
    ReferenceUse,
};
use trpg_shared_kernel::TrpgError;

#[test]
fn open_source_reference_matrix_accepts_reviewed_permissive_reference() {
    let entry = ReferenceEntry {
        name: "rust".to_owned(),
        provenance_url: "https://www.rust-lang.org/".to_owned(),
        license_policy: LicensePolicy::Permissive,
        intended_use: ReferenceUse::RuntimeDependency,
        reviewed: true,
    };

    validate_reference_entry(&entry).unwrap();
}

#[test]
fn open_source_reference_matrix_requires_review_for_non_permissive_use() {
    let entry = ReferenceEntry {
        name: "reciprocal-lib".to_owned(),
        provenance_url: "https://example.invalid/reference".to_owned(),
        license_policy: LicensePolicy::ReciprocalReviewRequired,
        intended_use: ReferenceUse::RuntimeDependency,
        reviewed: false,
    };

    assert!(matches!(
        validate_reference_entry(&entry),
        Err(TrpgError::OpenSourceReferenceViolation(_))
    ));
}

#[test]
fn open_source_reference_matrix_requires_level_four_local_ai_keeper() {
    assert!(matches!(
        validate_local_model_for_ai_keeper(LocalModelCertification::Level3),
        Err(TrpgError::OpenSourceReferenceViolation(_))
    ));

    validate_local_model_for_ai_keeper(LocalModelCertification::Level4).unwrap();
}
