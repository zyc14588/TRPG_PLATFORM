use trpg_agent_runtime::agent_pack_sdk::{self, AgentPackManifest};
use trpg_agent_runtime::{AgentKind, AgentTool, AuthorityMode, ToolRequest, VisibilityLabel};

fn manifest() -> AgentPackManifest {
    AgentPackManifest {
        pack_id: "coc7_keeper_pack",
        prompt_version: "keeper_prompt_v1",
        tool_schema_version: "agent_tool_schema_current",
        allowed_tools: vec![
            AgentTool::RequestSkillCheck,
            AgentTool::ApplySanLoss,
            AgentTool::DraftSanLoss,
        ],
        allowed_visibility: vec![VisibilityLabel::Public, VisibilityLabel::KeeperOnly],
    }
}

#[test]
fn agent_pack_sdk_requires_current_safe_manifest_and_tool_grant() {
    let manifest = manifest();
    assert!(manifest.is_current_safe());

    let denied_request =
        ToolRequest::formal(AgentKind::AiKeeperOrchestrator, AgentTool::RevealClue);
    let denied = agent_pack_sdk::evaluate_agent_pack_tool_request(
        &AuthorityMode::AiKp,
        &manifest,
        &denied_request,
    );
    assert_eq!(denied.error, Some("TOOL_PERMISSION_DENIED"));

    let allowed_request = ToolRequest::formal(
        AgentKind::AiKeeperOrchestrator,
        AgentTool::RequestSkillCheck,
    );
    let allowed = agent_pack_sdk::evaluate_agent_pack_tool_request(
        &AuthorityMode::AiKp,
        &manifest,
        &allowed_request,
    );
    assert!(allowed.tool_executed);
    assert!(allowed.error.is_none());
}

#[test]
fn agent_pack_sdk_preserves_human_kp_draft_only_boundary() {
    let manifest = manifest();
    let request = ToolRequest::formal(AgentKind::KeeperCopilot, AgentTool::ApplySanLoss);

    let decision = agent_pack_sdk::evaluate_agent_pack_tool_request(
        &AuthorityMode::HumanKp,
        &manifest,
        &request,
    );

    assert!(!decision.tool_executed);
    assert_eq!(decision.downgraded_to, Some(AgentTool::DraftSanLoss));
    assert!(decision.requires_human_confirmation);
    assert!(decision.draft_only);
    assert_eq!(decision.error, Some("HUMAN_KP_AI_DRAFT_ONLY"));
}
