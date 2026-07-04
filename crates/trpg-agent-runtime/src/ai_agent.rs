use crate::agent_runtime::{commit_agent_decision, AgentDecision, AgentEventPayload, AgentResult};
use trpg_shared_kernel::{AuthorityContract, CommandEnvelope, EventEnvelope, EventStore};

pub const PROMPT_ID: &str = "CODEX-0470-04-AI-AGENT-SYSTEM-01fd0c2f41";

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
    store: &mut EventStore<AgentEventPayload>,
    contract: &AuthorityContract,
    command: &CommandEnvelope<AgentDecision>,
    decision: AgentDecision,
) -> AgentResult<Vec<EventEnvelope<AgentEventPayload>>> {
    commit_agent_decision(store, contract, command, decision)
}
