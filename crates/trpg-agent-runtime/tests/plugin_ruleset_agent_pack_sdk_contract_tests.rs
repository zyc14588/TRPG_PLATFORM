use trpg_agent_runtime::agent_pack_sdk::AgentPackManifest;
use trpg_agent_runtime::plugin_ruleset_agent_pack_sdk::{self, PluginRulesetAgentPackPolicy};
use trpg_agent_runtime::{AgentKind, AgentTool, AuthorityMode, ToolRequest, VisibilityLabel};

fn policy(gateway_entrypoint: &'static str) -> PluginRulesetAgentPackPolicy {
    PluginRulesetAgentPackPolicy {
        plugin_id: "investigation_tools",
        ruleset_id: "coc7",
        gateway_entrypoint,
        manifest: AgentPackManifest {
            pack_id: "coc7_keeper_pack",
            prompt_id: plugin_ruleset_agent_pack_sdk::PROMPT_ID,
            tool_schema_version: "agent_tool_schema_current",
            allowed_tools: vec![AgentTool::RevealClue, AgentTool::RequestSkillCheck],
            allowed_visibility: vec![VisibilityLabel::Public],
        },
    }
}

#[test]
fn plugin_ruleset_agent_pack_sdk_requires_agent_gateway_scope() {
    assert_eq!(
        plugin_ruleset_agent_pack_sdk::PROMPT_ID,
        "CODEX-0479-04-AI-AGENT-SYSTEM-f4f075147a"
    );

    let request = ToolRequest::formal(
        AgentKind::AiKeeperOrchestrator,
        AgentTool::RequestSkillCheck,
    );
    let denied = plugin_ruleset_agent_pack_sdk::evaluate_plugin_ruleset_tool_request(
        &AuthorityMode::AiKp,
        &policy("provider_adapter_direct"),
        &request,
    );

    assert_eq!(denied.error, Some("ToolPermissionDenied"));
}

#[test]
fn plugin_ruleset_agent_pack_sdk_cannot_bypass_runtime_tool_gate() {
    let request = ToolRequest::formal(AgentKind::AtmosphereWriter, AgentTool::RevealClue);

    let denied = plugin_ruleset_agent_pack_sdk::evaluate_plugin_ruleset_tool_request(
        &AuthorityMode::AiKp,
        &policy("Agent Gateway"),
        &request,
    );

    assert_eq!(denied.error, Some("ToolPermissionDenied"));
    assert!(!denied.tool_executed);
}
