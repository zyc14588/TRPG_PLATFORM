use crate::model_provider::provider_boundary_snapshot;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentRuntimeReadmeSnapshot {
    pub ai_entrypoint: &'static str,
    pub runtime_boundary: &'static str,
    pub provider_adapter: &'static str,
    pub formal_state_policy: &'static str,
    pub visibility_policy: &'static str,
    pub fact_provenance_policy: &'static str,
    pub forbidden_direct_call_error: &'static str,
}

pub fn readme_governance_snapshot() -> AgentRuntimeReadmeSnapshot {
    let boundary = provider_boundary_snapshot();

    AgentRuntimeReadmeSnapshot {
        ai_entrypoint: boundary.gateway,
        runtime_boundary: boundary.runtime,
        provider_adapter: boundary.provider_adapter,
        formal_state_policy: "Agents propose; workflows commit through Event Store",
        visibility_policy:
            "Visibility labels propagate to agent context, events, RAG, replay, export, and logs",
        fact_provenance_policy: "Fact provenance is copied from command envelope to event envelope",
        forbidden_direct_call_error: boundary.forbidden_direct_call_error,
    }
}
