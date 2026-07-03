use crate::constitution::{validate_constitution_checklist, ConstitutionChecklist};
use crate::shared_kernel::{CommandEnvelope, EventEnvelope, EventStore, KernelResult};
use crate::workspace_and_governance::{
    append_governance_reviewed, validate_governance_contract, GovernanceContract, GovernanceReview,
    GovernanceReviewedPayload, GovernanceSurface, REQUIRED_COMMAND_FIELDS,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConstitutionLanding {
    pub checklist: ConstitutionChecklist,
    pub governance_contract: GovernanceContract,
}

pub fn constitution_impl_contract() -> GovernanceContract {
    GovernanceContract {
        module_name: "constitution_impl",
        source_file: "crates/trpg-shared-kernel/src/constitution_impl.rs",
        test_file: "crates/trpg-shared-kernel/tests/constitution_impl_contract_tests.rs",
        surface: GovernanceSurface::ConstitutionImplementation,
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
