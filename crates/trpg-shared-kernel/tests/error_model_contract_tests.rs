use trpg_shared_kernel::error_model::{compose_error, describe_error};
use trpg_shared_kernel::{TrpgError, WireErrorCode};

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
    let descriptor = compose_error(WireErrorCode::UnknownVisibilityLabel, false);

    assert_eq!(descriptor.code, "UNKNOWN_VISIBILITY_LABEL");
    assert!(descriptor.visibility_safe);
}

#[test]
fn every_shared_kernel_error_uses_the_canonical_wire_registry() {
    let cases = [
        (TrpgError::InvalidEntityId, WireErrorCode::InvalidEntityId),
        (
            TrpgError::UnknownVisibilityLabel,
            WireErrorCode::UnknownVisibilityLabel,
        ),
        (
            TrpgError::MissingIdempotencyKey,
            WireErrorCode::MissingIdempotencyKey,
        ),
        (
            TrpgError::MissingCorrelationId,
            WireErrorCode::MissingCorrelationId,
        ),
        (
            TrpgError::MissingCausationId,
            WireErrorCode::MissingCausationId,
        ),
        (
            TrpgError::MissingFactProvenance,
            WireErrorCode::MissingFactProvenance,
        ),
        (
            TrpgError::AuthorityViolation,
            WireErrorCode::AuthorityViolation,
        ),
        (
            TrpgError::AuthorityContractMutation,
            WireErrorCode::AuthorityContractMutation,
        ),
        (
            TrpgError::DirectAgentStateWrite,
            WireErrorCode::DirectAgentStateWrite,
        ),
        (TrpgError::PolicyDenied, WireErrorCode::PolicyDenied),
        (
            TrpgError::ExpectedVersionConflict {
                expected: 1,
                actual: 2,
            },
            WireErrorCode::ExpectedVersionConflict,
        ),
        (TrpgError::DuplicateCommand, WireErrorCode::DuplicateCommand),
        (TrpgError::VisibilityDenied, WireErrorCode::VisibilityDenied),
        (
            TrpgError::InvalidConfiguration("test"),
            WireErrorCode::InvalidConfiguration,
        ),
        (
            TrpgError::DependencyViolation("test"),
            WireErrorCode::DependencyDirectionViolation,
        ),
        (
            TrpgError::CrateOwnershipViolation("test"),
            WireErrorCode::CrateOwnershipViolation,
        ),
        (
            TrpgError::WorkspaceViolation("test"),
            WireErrorCode::WorkspaceContractViolation,
        ),
        (
            TrpgError::CodingPolicyViolation("test"),
            WireErrorCode::RustCodingPolicyViolation,
        ),
        (
            TrpgError::OpenSourceReferenceViolation("test"),
            WireErrorCode::OpenSourceReferenceViolation,
        ),
    ];

    assert_eq!(cases.len(), 19);
    for (error, expected) in cases {
        assert_eq!(error.wire_code(), expected);
        assert_eq!(error.code(), expected.as_str());
        assert_eq!(WireErrorCode::lookup(error.code()), Ok(expected));
    }
}
