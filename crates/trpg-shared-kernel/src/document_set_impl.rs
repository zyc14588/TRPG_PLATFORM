use crate::constitution::ConstitutionChecklist;
use crate::document_set::validate_governance_checklist;
use crate::shared_kernel::{CommandEnvelope, EventEnvelope, EventStore, KernelResult};
use crate::workspace_and_governance::{
    append_governance_reviewed, validate_governance_contract, GovernanceContract, GovernanceReview,
    GovernanceReviewedPayload, GovernanceSurface,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DocumentSetLanding {
    pub checklist: ConstitutionChecklist,
    pub governance_contract: GovernanceContract,
}

pub fn document_set_impl_contract() -> GovernanceContract {
    GovernanceContract::new(
        "document_set_impl",
        GovernanceSurface::DocumentSetImplementation,
    )
}

pub fn document_set_landing(checklist: ConstitutionChecklist) -> DocumentSetLanding {
    DocumentSetLanding {
        checklist,
        governance_contract: document_set_impl_contract(),
    }
}

pub fn validate_document_set_landing(landing: &DocumentSetLanding) -> KernelResult<()> {
    validate_governance_contract(&landing.governance_contract)?;
    validate_governance_checklist(&landing.checklist)
}

pub fn document_set_impl_review() -> GovernanceReview {
    GovernanceReview {
        contract: document_set_impl_contract(),
        checked_requirements: vec![
            "governance_checklist_is_complete",
            "product_boundary_landing_preserves_event_store_canon",
        ],
    }
}

pub fn append_document_set_impl_reviewed(
    store: &mut EventStore<GovernanceReviewedPayload>,
    command: &CommandEnvelope<GovernanceReview>,
) -> KernelResult<EventEnvelope<GovernanceReviewedPayload>> {
    append_governance_reviewed(store, command)
}
