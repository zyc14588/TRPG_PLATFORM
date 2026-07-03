use trpg_shared_kernel::error_model::{compose_error, describe_error};
use trpg_shared_kernel::TrpgError;

#[test]
fn error_model_exposes_stable_error_codes() {
    let descriptor = describe_error(&TrpgError::InvalidEntityId);

    assert_eq!(descriptor.code, "INVALID_ENTITY_ID");
    assert!(!descriptor.retryable);
    assert!(descriptor.visibility_safe);
}

#[test]
fn error_model_marks_concurrency_errors_retryable() {
    let descriptor = describe_error(&TrpgError::ExpectedVersionConflict {
        expected: 0,
        actual: 1,
    });

    assert_eq!(descriptor.code, "EXPECTED_VERSION_CONFLICT");
    assert!(descriptor.retryable);
}

#[test]
fn error_model_composes_fixture_error_descriptor() {
    let descriptor = compose_error("UNKNOWN_VISIBILITY_LABEL", false);

    assert_eq!(descriptor.code, "UNKNOWN_VISIBILITY_LABEL");
    assert!(descriptor.visibility_safe);
}
