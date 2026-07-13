use crate::shared_kernel::{CommandEnvelope, EventEnvelope, EventStore, KernelResult, TrpgError};
use crate::workspace_and_governance::{
    append_governance_reviewed, validate_governance_contract, GovernanceContract, GovernanceReview,
    GovernanceReviewedPayload, GovernanceSurface,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContextPropagationChannel {
    Api,
    Event,
    AgentContext,
    ToolResult,
    Rag,
    Summary,
    Export,
    Replay,
    Log,
    Metric,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemContextPolicy {
    pub channels: Vec<ContextPropagationChannel>,
    pub direct_model_provider_access: bool,
    pub direct_agent_state_write: bool,
    pub formal_decisions_use_event_store: bool,
}

pub fn system_context_contract() -> GovernanceContract {
    GovernanceContract::new("system_context", GovernanceSurface::SystemContext)
}

pub fn current_system_context_policy() -> SystemContextPolicy {
    SystemContextPolicy {
        channels: vec![
            ContextPropagationChannel::Api,
            ContextPropagationChannel::Event,
            ContextPropagationChannel::AgentContext,
            ContextPropagationChannel::ToolResult,
            ContextPropagationChannel::Rag,
            ContextPropagationChannel::Summary,
            ContextPropagationChannel::Export,
            ContextPropagationChannel::Replay,
            ContextPropagationChannel::Log,
            ContextPropagationChannel::Metric,
        ],
        direct_model_provider_access: false,
        direct_agent_state_write: false,
        formal_decisions_use_event_store: true,
    }
}

pub fn validate_system_context_policy(policy: &SystemContextPolicy) -> KernelResult<()> {
    validate_governance_contract(&system_context_contract())?;

    if policy.direct_model_provider_access {
        return Err(TrpgError::PolicyDenied);
    }

    if policy.direct_agent_state_write {
        return Err(TrpgError::DirectAgentStateWrite);
    }

    if !policy.formal_decisions_use_event_store {
        return Err(TrpgError::WorkspaceViolation(
            "formal decisions must use the event store",
        ));
    }

    for channel in [
        ContextPropagationChannel::Api,
        ContextPropagationChannel::Event,
        ContextPropagationChannel::AgentContext,
        ContextPropagationChannel::ToolResult,
        ContextPropagationChannel::Rag,
        ContextPropagationChannel::Summary,
        ContextPropagationChannel::Export,
        ContextPropagationChannel::Replay,
        ContextPropagationChannel::Log,
        ContextPropagationChannel::Metric,
    ] {
        if !policy.channels.contains(&channel) {
            return Err(TrpgError::MissingFactProvenance);
        }
    }

    Ok(())
}

pub fn system_context_review() -> GovernanceReview {
    GovernanceReview {
        contract: system_context_contract(),
        checked_requirements: vec![
            "visibility_and_fact_provenance_cross_all_context_channels",
            "agent_context_uses_gateway_runtime_and_tools",
            "formal_decisions_reach_event_store",
        ],
    }
}

pub fn append_system_context_reviewed(
    store: &mut EventStore<GovernanceReviewedPayload>,
    command: &CommandEnvelope<GovernanceReview>,
) -> KernelResult<EventEnvelope<GovernanceReviewedPayload>> {
    append_governance_reviewed(store, command)
}
