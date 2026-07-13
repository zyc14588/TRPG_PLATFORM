use trpg_domain_core::ddd::TrpgError;
use trpg_domain_core::{DomainError, WireErrorCode};

fn expected_wire_code(error: &DomainError) -> WireErrorCode {
    match error {
        DomainError::AuthorityContractImmutable => WireErrorCode::AuthorityContractImmutable,
        DomainError::AuthorityViolation => WireErrorCode::AuthorityViolation,
        DomainError::InvalidConfirmedFactSource => WireErrorCode::InvalidConfirmedFactSource,
        DomainError::MissingCommandMetadata => WireErrorCode::MissingCommandMetadata,
        DomainError::DuplicateCommand => WireErrorCode::DuplicateCommand,
        DomainError::ExpectedVersionConflict { .. } => WireErrorCode::ExpectedVersionConflict,
        DomainError::VisibilityDenied => WireErrorCode::VisibilityDenied,
        DomainError::PolicyDenied => WireErrorCode::PolicyDenied,
        DomainError::SharedKernel(code) => *code,
    }
}

#[test]
fn every_fixed_domain_error_uses_its_canonical_wire_code() {
    let errors = [
        DomainError::AuthorityContractImmutable,
        DomainError::AuthorityViolation,
        DomainError::InvalidConfirmedFactSource,
        DomainError::MissingCommandMetadata,
        DomainError::DuplicateCommand,
        DomainError::ExpectedVersionConflict {
            expected: 2,
            actual: 1,
        },
        DomainError::VisibilityDenied,
        DomainError::PolicyDenied,
    ];

    for error in errors {
        let expected = expected_wire_code(&error);
        assert_eq!(error.wire_code(), expected);
        assert_eq!(error.code(), expected.as_str());
        assert_eq!(WireErrorCode::lookup(error.code()), Ok(expected));
        assert_eq!(error.to_string(), expected.as_str());
    }
}

#[test]
fn every_dynamic_shared_kernel_error_is_registry_typed() {
    for code in WireErrorCode::ALL {
        let error = DomainError::SharedKernel(*code);
        assert_eq!(expected_wire_code(&error), *code);
        assert_eq!(error.wire_code(), *code);
        assert_eq!(WireErrorCode::lookup(error.code()), Ok(*code));
    }
}

#[test]
fn shared_kernel_conversion_preserves_the_canonical_registry_value() {
    assert_eq!(
        DomainError::from(TrpgError::InvalidEntityId),
        DomainError::SharedKernel(WireErrorCode::InvalidEntityId)
    );
    assert_eq!(
        DomainError::from(TrpgError::MissingFactProvenance),
        DomainError::SharedKernel(WireErrorCode::MissingFactProvenance)
    );
}
