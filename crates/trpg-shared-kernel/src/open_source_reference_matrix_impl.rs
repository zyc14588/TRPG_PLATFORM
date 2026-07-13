use crate::open_source_reference_matrix::{validate_reference_entry, ReferenceEntry};
use crate::shared_kernel::{CommandEnvelope, EventEnvelope, EventStore, KernelResult, TrpgError};
use crate::workspace_and_governance::{
    append_governance_reviewed, validate_governance_contract, GovernanceContract, GovernanceReview,
    GovernanceReviewedPayload, GovernanceSurface,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenSourceReferenceMatrixLanding {
    pub references: Vec<ReferenceEntry>,
    pub governance_contract: GovernanceContract,
}

pub fn open_source_reference_matrix_impl_contract() -> GovernanceContract {
    GovernanceContract::new(
        "open_source_reference_matrix_impl",
        GovernanceSurface::OpenSourceReferenceMatrixImplementation,
    )
}

pub fn open_source_reference_matrix_landing(
    references: Vec<ReferenceEntry>,
) -> OpenSourceReferenceMatrixLanding {
    OpenSourceReferenceMatrixLanding {
        references,
        governance_contract: open_source_reference_matrix_impl_contract(),
    }
}

pub fn validate_open_source_reference_matrix_landing(
    landing: &OpenSourceReferenceMatrixLanding,
) -> KernelResult<()> {
    validate_governance_contract(&landing.governance_contract)?;

    if landing.references.is_empty() {
        return Err(TrpgError::OpenSourceReferenceViolation(
            "reference matrix requires at least one reviewed reference",
        ));
    }

    for reference in &landing.references {
        validate_reference_entry(reference)?;
    }

    Ok(())
}

pub fn open_source_reference_matrix_impl_review() -> GovernanceReview {
    GovernanceReview {
        contract: open_source_reference_matrix_impl_contract(),
        checked_requirements: vec![
            "reference_entries_keep_provenance",
            "non_permissive_references_require_review",
            "local_model_certification_boundary_is_explicit",
        ],
    }
}

pub fn append_open_source_reference_matrix_impl_reviewed(
    store: &mut EventStore<GovernanceReviewedPayload>,
    command: &CommandEnvelope<GovernanceReview>,
) -> KernelResult<EventEnvelope<GovernanceReviewedPayload>> {
    append_governance_reviewed(store, command)
}
