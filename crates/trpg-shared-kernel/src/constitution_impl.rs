use crate::constitution::{validate_constitution_checklist, ConstitutionChecklist};
use crate::shared_kernel::{CommandEnvelope, EventEnvelope, EventStore, KernelResult};
use crate::workspace_and_governance::{
    append_governance_reviewed, validate_governance_contract, GovernanceContract, GovernanceReview,
    GovernanceReviewedPayload, GovernanceSurface,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConstitutionLanding {
    pub checklist: ConstitutionChecklist,
    pub governance_contract: GovernanceContract,
}

pub fn constitution_impl_contract() -> GovernanceContract {
    GovernanceContract::new(
        "constitution_impl",
        GovernanceSurface::ConstitutionImplementation,
    )
}

pub fn constitution_landing(checklist: ConstitutionChecklist) -> ConstitutionLanding {
    ConstitutionLanding {
        checklist,
        governance_contract: constitution_impl_contract(),
    }
}

pub fn validate_constitution_landing(landing: &ConstitutionLanding) -> KernelResult<()> {
    validate_governance_contract(&landing.governance_contract)?;
    validate_constitution_checklist(&landing.checklist)
}

pub fn constitution_impl_review() -> GovernanceReview {
    GovernanceReview {
        contract: constitution_impl_contract(),
        checked_requirements: vec![
            "constitution_articles_are_current",
            "constitution_landing_preserves_governance_boundaries",
        ],
    }
}

pub fn append_constitution_impl_reviewed(
    store: &mut EventStore<GovernanceReviewedPayload>,
    command: &CommandEnvelope<GovernanceReview>,
) -> KernelResult<EventEnvelope<GovernanceReviewedPayload>> {
    append_governance_reviewed(store, command)
}
