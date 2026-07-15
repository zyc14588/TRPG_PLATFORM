#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentGovernanceSnapshot {
    pub ai_entrypoint: &'static str,
    pub formal_state_policy: &'static str,
    pub tool_gate_policy: &'static str,
    pub visibility_policy: &'static str,
}

pub fn agent_governance_snapshot() -> AgentGovernanceSnapshot {
    AgentGovernanceSnapshot {
        ai_entrypoint: "Agent Gateway -> Agent Orchestrator/Runtime -> Model Provider Adapter",
        formal_state_policy: "Agent output is Proposal, ToolCall, or DraftDecision only",
        tool_gate_policy: "default deny",
        visibility_policy: "derived visibility cannot exceed source visibility",
    }
}
