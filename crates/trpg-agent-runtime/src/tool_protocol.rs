use crate::agent_runtime::{evaluate_agent_tool_request, ToolDecision, ToolRequest};
use trpg_shared_kernel::AuthorityMode;

pub const PROMPT_ID: &str = "CODEX-0047-04-AI-AGENT-SYSTEM-524e2550a8";

pub fn decide_tool_request(authority_mode: &AuthorityMode, request: &ToolRequest) -> ToolDecision {
    evaluate_agent_tool_request(authority_mode, request)
}
