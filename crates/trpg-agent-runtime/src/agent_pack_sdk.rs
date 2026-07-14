use crate::agent_runtime::{
    evaluate_agent_tool_request, AgentError, AgentTool, ToolDecision, ToolRequest,
};
use trpg_shared_kernel::{AuthorityMode, VisibilityLabel};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentPackManifest {
    pub pack_id: &'static str,
    pub tool_schema_version: &'static str,
    pub allowed_tools: Vec<AgentTool>,
    pub allowed_visibility: Vec<VisibilityLabel>,
}

impl AgentPackManifest {
    pub fn is_current_safe(&self) -> bool {
        !self.pack_id.trim().is_empty()
            && !self.tool_schema_version.trim().is_empty()
            && !self.allowed_tools.is_empty()
            && !self.allowed_visibility.is_empty()
    }

    pub fn allows_tool_request(&self, request: &ToolRequest) -> bool {
        self.allowed_tools.contains(&request.tool())
            && self
                .allowed_visibility
                .iter()
                .any(|label| label == request.visibility().label())
    }
}

fn deny_tool() -> ToolDecision {
    ToolDecision {
        tool_executed: false,
        downgraded_to: None,
        requires_human_confirmation: false,
        draft_only: false,
        error: Some(AgentError::ToolPermissionDenied.code()),
    }
}

pub fn evaluate_agent_pack_tool_request(
    authority_mode: &AuthorityMode,
    manifest: &AgentPackManifest,
    request: &ToolRequest,
) -> ToolDecision {
    if !manifest.is_current_safe() || !manifest.allows_tool_request(request) {
        return deny_tool();
    }

    evaluate_agent_tool_request(authority_mode, request)
}
