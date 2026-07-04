use crate::agent_runtime::{evaluate_agent_tool_request, ToolDecision, ToolRequest};
use trpg_shared_kernel::AuthorityMode;

pub const PROMPT_ID: &str = "CODEX-0444-04-AI-AGENT-SYSTEM-20434579ca";

pub fn runtime_tool_gate(authority_mode: &AuthorityMode, request: &ToolRequest) -> ToolDecision {
    evaluate_agent_tool_request(authority_mode, request)
}
