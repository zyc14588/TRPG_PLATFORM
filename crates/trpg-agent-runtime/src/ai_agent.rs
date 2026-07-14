use crate::agent_runtime::{AgentDecision, AgentDecisionCommitter, AgentEventPayload, AgentResult};
use trpg_shared_kernel::{CommandEnvelope, EventEnvelope, EventStore};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AiAgentBoundary {
    pub ai_entrypoint: &'static str,
    pub formal_state_path: &'static str,
    pub event_log_boundary: &'static str,
    pub direct_provider_call_policy: &'static str,
}

pub fn ai_agent_boundary() -> AiAgentBoundary {
    AiAgentBoundary {
        ai_entrypoint: "Agent Gateway",
        formal_state_path: "Command -> Workflow -> Decision -> Event Store",
        event_log_boundary: "Event Store is canon; projections are rebuildable",
        direct_provider_call_policy: "Direct LLM/provider calls are forbidden",
    }
}

pub fn submit_ai_agent_decision(
    committer: &AgentDecisionCommitter,
    store: &mut EventStore<AgentEventPayload>,
    command: &CommandEnvelope<AgentDecision>,
    decision: AgentDecision,
    now_unix_ms: u64,
) -> AgentResult<Vec<EventEnvelope<AgentEventPayload>>> {
    committer.commit(store, command, decision, now_unix_ms)
}
