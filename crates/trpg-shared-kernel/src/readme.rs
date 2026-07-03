use crate::shared_kernel::{CommandEnvelope, EventEnvelope, EventStore, KernelResult, TrpgError};
use crate::workspace_and_governance::{
    append_governance_reviewed, validate_governance_contract, GovernanceContract, GovernanceReview,
    GovernanceReviewedPayload, GovernanceSurface, REQUIRED_COMMAND_FIELDS,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReadmeContract {
    pub points_to_top_level_design: bool,
    pub points_to_bootstrap_prompt: bool,
    pub points_to_normalized_maps: bool,
    pub states_historical_inputs_are_provenance_only: bool,
    pub forbids_direct_model_provider_access: bool,
}

pub fn readme_contract() -> GovernanceContract {
    GovernanceContract {
        module_name: "readme",
        source_file: "crates/trpg-shared-kernel/src/readme.rs",
        test_file: "crates/trpg-shared-kernel/tests/readme_contract_tests.rs",
        surface: GovernanceSurface::Readme,
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

pub fn current_readme_contract() -> ReadmeContract {
    ReadmeContract {
        points_to_top_level_design: true,
        points_to_bootstrap_prompt: true,
        points_to_normalized_maps: true,
        states_historical_inputs_are_provenance_only: true,
        forbids_direct_model_provider_access: true,
    }
}

pub fn validate_readme_contract(contract: &ReadmeContract) -> KernelResult<()> {
    validate_governance_contract(&readme_contract())?;

    if !contract.points_to_top_level_design
        || !contract.points_to_bootstrap_prompt
        || !contract.points_to_normalized_maps
        || !contract.states_historical_inputs_are_provenance_only
        || !contract.forbids_direct_model_provider_access
    {
        return Err(TrpgError::WorkspaceViolation(
            "readme must preserve foundation governance entry points",
        ));
    }

    Ok(())
}

pub fn readme_review() -> GovernanceReview {
    GovernanceReview {
        contract: readme_contract(),
        checked_requirements: vec![
            "readme_points_to_current_authority_documents",
            "readme_keeps_historical_inputs_as_provenance",
            "readme_preserves_agent_gateway_boundary",
        ],
    }
}

pub fn append_readme_reviewed(
    store: &mut EventStore<GovernanceReviewedPayload>,
    command: &CommandEnvelope<GovernanceReview>,
) -> KernelResult<EventEnvelope<GovernanceReviewedPayload>> {
    append_governance_reviewed(store, command)
}
