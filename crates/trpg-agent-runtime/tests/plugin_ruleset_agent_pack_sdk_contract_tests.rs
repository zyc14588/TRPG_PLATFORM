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
            tool_schema_version: "agent_tool_schema_current",
            allowed_tools: vec![AgentTool::RevealClue, AgentTool::RequestSkillCheck],
            allowed_visibility: vec![VisibilityLabel::Public],
        },
    }
}

#[test]
fn plugin_ruleset_agent_pack_sdk_requires_agent_gateway_scope() {
    assert_eq!(
        trpg_test_support::normalized_prompt_id(
            "trpg-agent-runtime",
            "plugin_ruleset_agent_pack_sdk"
        ),
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

    assert_eq!(denied.error, Some("TOOL_PERMISSION_DENIED"));
}

#[test]
fn plugin_ruleset_agent_pack_sdk_cannot_bypass_runtime_tool_gate() {
    let request = ToolRequest::formal(AgentKind::AtmosphereWriter, AgentTool::RevealClue);

    let denied = plugin_ruleset_agent_pack_sdk::evaluate_plugin_ruleset_tool_request(
        &AuthorityMode::AiKp,
        &policy("Agent Gateway"),
        &request,
    );

    assert_eq!(denied.error, Some("TOOL_PERMISSION_DENIED"));
    assert!(!denied.tool_executed);
}
