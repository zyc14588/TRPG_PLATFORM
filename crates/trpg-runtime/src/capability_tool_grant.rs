use crate::runtime_state_machines::{
    approve_tool_request, RuntimeResult, ToolGrantDecision, ToolRequest,
};
use trpg_shared_kernel::AuthorityMode;

pub const PROMPT_ID: &str = "CODEX-0032-03-RUNTIME-ORCHESTRATION-20830a72ac";

pub fn grant_tool(
    authority_mode: &AuthorityMode,
    request: &ToolRequest,
) -> RuntimeResult<ToolGrantDecision> {
    approve_tool_request(authority_mode, request)
}
