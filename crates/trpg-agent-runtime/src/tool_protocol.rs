use crate::agent_runtime::{evaluate_agent_tool_request, ToolDecision, ToolRequest};
use trpg_shared_kernel::AuthorityMode;

pub fn decide_tool_request(authority_mode: &AuthorityMode, request: &ToolRequest) -> ToolDecision {
    evaluate_agent_tool_request(authority_mode, request)
}
