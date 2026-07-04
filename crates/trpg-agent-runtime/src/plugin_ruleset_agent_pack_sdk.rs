use crate::agent_pack_sdk::{evaluate_agent_pack_tool_request, AgentPackManifest};
use crate::agent_runtime::{AgentError, ToolDecision, ToolRequest};
use trpg_shared_kernel::AuthorityMode;

pub const PROMPT_ID: &str = "CODEX-0479-04-AI-AGENT-SYSTEM-f4f075147a";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginRulesetAgentPackPolicy {
    pub plugin_id: &'static str,
    pub ruleset_id: &'static str,
    pub gateway_entrypoint: &'static str,
    pub manifest: AgentPackManifest,
}

impl PluginRulesetAgentPackPolicy {
    pub fn is_gateway_scoped(&self) -> bool {
        !self.plugin_id.trim().is_empty()
            && !self.ruleset_id.trim().is_empty()
            && self.gateway_entrypoint == "Agent Gateway"
            && self.manifest.is_current_safe()
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

pub fn evaluate_plugin_ruleset_tool_request(
    authority_mode: &AuthorityMode,
    policy: &PluginRulesetAgentPackPolicy,
    request: &ToolRequest,
) -> ToolDecision {
    if !policy.is_gateway_scoped() {
        return deny_tool();
    }

    evaluate_agent_pack_tool_request(authority_mode, &policy.manifest, request)
}
