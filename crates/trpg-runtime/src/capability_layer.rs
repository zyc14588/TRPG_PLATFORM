use crate::runtime_state_machines::{evaluate_tool_grant, ToolGrantDecision, ToolRequest};
use trpg_shared_kernel::AuthorityMode;

pub fn evaluate_capability_layer(
    authority_mode: &AuthorityMode,
    request: &ToolRequest,
) -> ToolGrantDecision {
    evaluate_tool_grant(authority_mode, request)
}
