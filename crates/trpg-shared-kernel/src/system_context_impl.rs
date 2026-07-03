use crate::shared_kernel::{CommandEnvelope, EventEnvelope, EventStore, KernelResult};
use crate::system_context::{validate_system_context_policy, SystemContextPolicy};
use crate::workspace_and_governance::{
    append_governance_reviewed, validate_governance_contract, CanonicalStateBoundary,
    GovernanceContract, GovernanceReview, GovernanceReviewedPayload, GovernanceSurface,
    REQUIRED_COMMAND_FIELDS,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemContextLanding {
    pub policy: SystemContextPolicy,
    pub governance_contract: GovernanceContract,
}

pub fn system_context_impl_contract() -> GovernanceContract {
    GovernanceContract {
        module_name: "system_context_impl",
        source_file: "crates/trpg-shared-kernel/src/system_context_impl.rs",
        test_file: "crates/trpg-shared-kernel/tests/system_context_impl_contract_tests.rs",
        surface: GovernanceSurface::SystemContextImplementation,
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

pub fn system_context_landing(policy: SystemContextPolicy) -> SystemContextLanding {
    SystemContextLanding {
        policy,
        governance_contract: system_context_impl_contract(),
    }
}

pub fn validate_system_context_landing(landing: &SystemContextLanding) -> KernelResult<()> {
    validate_governance_contract(&landing.governance_contract)?;
    validate_system_context_policy(&landing.policy)
}

pub fn system_context_impl_review() -> GovernanceReview {
    GovernanceReview {
        contract: system_context_impl_contract(),
        checked_requirements: vec![
            "all_context_channels_propagate_visibility",
            "agent_gateway_boundary_is_preserved",
            "formal_decisions_remain_event_sourced",
        ],
    }
}

pub fn append_system_context_impl_reviewed(
    store: &mut EventStore<GovernanceReviewedPayload>,
    command: &CommandEnvelope<GovernanceReview>,
) -> KernelResult<EventEnvelope<GovernanceReviewedPayload>> {
    append_governance_reviewed(store, command)
}
