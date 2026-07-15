use crate::adr_0009_agent_governance_agent_governance::{
    agent_governance_snapshot, AgentGovernanceSnapshot,
};
use crate::agent_runtime::{
    evaluate_agent_tool_request, validate_agent_command, AgentError, AgentResult, ToolDecision,
    ToolRequest,
};
use trpg_shared_kernel::{AuthorityContract, CommandEnvelope};

pub fn current_agent_governance_snapshot() -> AgentGovernanceSnapshot {
    agent_governance_snapshot()
}

pub fn validate_governed_tool_request<T>(
    contract: &AuthorityContract,
    command: &CommandEnvelope<T>,
    request: &ToolRequest,
) -> AgentResult<ToolDecision> {
    validate_agent_command(contract, command)?;
    let decision = evaluate_agent_tool_request(&command.authority_mode, request);
    if decision.error.is_some() && !decision.draft_only {
        return Err(AgentError::ToolPermissionDenied);
    }

    Ok(decision)
}
