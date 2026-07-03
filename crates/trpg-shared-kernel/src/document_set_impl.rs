use crate::document_set::{validate_foundation_document_set, FoundationDocumentSet};
use crate::shared_kernel::{CommandEnvelope, EventEnvelope, EventStore, KernelResult};
use crate::workspace_and_governance::{
    append_governance_reviewed, validate_governance_contract, GovernanceContract, GovernanceReview,
    GovernanceReviewedPayload, GovernanceSurface, REQUIRED_COMMAND_FIELDS,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentSetLanding {
    pub document_set: FoundationDocumentSet,
    pub governance_contract: GovernanceContract,
}

pub fn document_set_impl_contract() -> GovernanceContract {
    GovernanceContract {
        module_name: "document_set_impl",
        source_file: "crates/trpg-shared-kernel/src/document_set_impl.rs",
        test_file: "crates/trpg-shared-kernel/tests/document_set_impl_contract_tests.rs",
        surface: GovernanceSurface::DocumentSetImplementation,
        command_fields: REQUIRED_COMMAND_FIELDS,
        requires_agent_gateway: true,
        permits_direct_model_provider_access: false,
        permits_direct_agent_state_write: false,
        permits_authority_contract_mutation: false,
        canonical_state_boundary:
            crate::workspace_and_governance::CanonicalStateBoundary::EventStore,
        read_models_rebuildable: true,
        propagates_visibility_and_provenance: true,
    }
}

pub fn document_set_landing(document_set: FoundationDocumentSet) -> DocumentSetLanding {
    DocumentSetLanding {
        document_set,
        governance_contract: document_set_impl_contract(),
    }
}

pub fn validate_document_set_landing(landing: &DocumentSetLanding) -> KernelResult<()> {
    validate_governance_contract(&landing.governance_contract)?;
    validate_foundation_document_set(&landing.document_set)
}

pub fn document_set_impl_review() -> GovernanceReview {
    GovernanceReview {
        contract: document_set_impl_contract(),
        checked_requirements: vec![
            "document_set_is_complete",
            "document_set_landing_preserves_provenance_boundary",
        ],
    }
}

pub fn append_document_set_impl_reviewed(
    store: &mut EventStore<GovernanceReviewedPayload>,
    command: &CommandEnvelope<GovernanceReview>,
) -> KernelResult<EventEnvelope<GovernanceReviewedPayload>> {
    append_governance_reviewed(store, command)
}
