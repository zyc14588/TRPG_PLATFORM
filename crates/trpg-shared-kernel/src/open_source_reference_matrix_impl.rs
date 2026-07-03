use crate::open_source_reference_matrix::{validate_reference_entry, ReferenceEntry};
use crate::shared_kernel::{CommandEnvelope, EventEnvelope, EventStore, KernelResult, TrpgError};
use crate::workspace_and_governance::{
    append_governance_reviewed, validate_governance_contract, CanonicalStateBoundary,
    GovernanceContract, GovernanceReview, GovernanceReviewedPayload, GovernanceSurface,
    REQUIRED_COMMAND_FIELDS,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenSourceReferenceMatrixLanding {
    pub references: Vec<ReferenceEntry>,
    pub governance_contract: GovernanceContract,
}

pub fn open_source_reference_matrix_impl_contract() -> GovernanceContract {
    GovernanceContract {
        module_name: "open_source_reference_matrix_impl",
        source_file: "crates/trpg-shared-kernel/src/open_source_reference_matrix_impl.rs",
        test_file:
            "crates/trpg-shared-kernel/tests/open_source_reference_matrix_impl_contract_tests.rs",
        surface: GovernanceSurface::OpenSourceReferenceMatrixImplementation,
        command_fields: REQUIRED_COMMAND_FIELDS,
        requires_agent_gateway: true,
        permits_direct_model_provider_access: false,
        permits_direct_agent_state_write: false,
        permits_authority_contract_mutation: false,
        canonical_state_boundary: CanonicalStateBoundary::EventStore,
        read_models_rebuildable: true,
        propagates_visibility_and_provenance: true,
    }
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
