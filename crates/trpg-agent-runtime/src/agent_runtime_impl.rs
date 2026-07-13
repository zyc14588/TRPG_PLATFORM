use crate::agent_runtime::{
    assemble_context, commit_agent_decision, AgentDecision, AgentEventPayload, AgentResult,
    AssembledAgentContext, ContextFact,
};
use trpg_shared_kernel::{
    AuthorityContract, CommandEnvelope, EventEnvelope, EventStore, PrincipalScope,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentRuntimeImplBoundary {
    pub command_boundary: &'static str,
    pub event_store_boundary: &'static str,
    pub projection_boundary: &'static str,
}

pub fn agent_runtime_impl_boundary() -> AgentRuntimeImplBoundary {
    AgentRuntimeImplBoundary {
        command_boundary:
            "Command envelopes carry authority, visibility, provenance, correlation, and causation",
        event_store_boundary: "Formal agent decisions append through Event Store only",
        projection_boundary:
            "Projection, cache, RAG index, and summaries are rebuildable read models",
    }
}

pub fn run_agent_runtime_decision(
    store: &mut EventStore<AgentEventPayload>,
    contract: &AuthorityContract,
    command: &CommandEnvelope<AgentDecision>,
    decision: AgentDecision,
) -> AgentResult<Vec<EventEnvelope<AgentEventPayload>>> {
    commit_agent_decision(store, contract, command, decision)
}

pub fn assemble_runtime_context(
    facts: &[ContextFact],
    principal: &PrincipalScope,
) -> AssembledAgentContext {
    assemble_context(facts, principal)
}
