use crate::agent_runtime::{
    assemble_context, AgentDecision, AgentDecisionCommitter, AgentEventPayload, AgentResult,
    AssembledAgentContext, ContextFact,
};
use trpg_shared_kernel::{CommandEnvelope, EventEnvelope, EventStore, PrincipalScope};

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
    committer: &AgentDecisionCommitter,
    store: &mut EventStore<AgentEventPayload>,
    command: &CommandEnvelope<AgentDecision>,
    decision: AgentDecision,
    now_unix_ms: u64,
) -> AgentResult<Vec<EventEnvelope<AgentEventPayload>>> {
    committer.commit(store, command, decision, now_unix_ms)
}

pub fn assemble_runtime_context(
    facts: &[ContextFact],
    principal: &PrincipalScope,
) -> AssembledAgentContext {
    assemble_context(facts, principal)
}
