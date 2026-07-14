use trpg_agent_runtime::adr_0009_agent_governance;
use trpg_agent_runtime::{
    ActorRole, AgentKind, AgentTool, AuthorityMode, FormalWritePath, ToolRequest,
};

#[test]
fn adr_0009_agent_governance_keeps_gateway_and_default_deny() {
    assert_eq!(
        trpg_test_support::normalized_prompt_id("trpg-agent-runtime", "adr_0009_agent_governance"),
        "CODEX-0507-04-AI-AGENT-SYSTEM-a1e5d3d499"
    );
    let snapshot = adr_0009_agent_governance::current_agent_governance_snapshot();

    assert!(snapshot.ai_entrypoint.contains("Agent Gateway"));
    assert!(snapshot.ai_entrypoint.contains("Model Provider Adapter"));
    assert_eq!(snapshot.tool_gate_policy, "default deny");
}

#[test]
fn adr_0009_agent_governance_blocks_direct_agent_write_before_tool_gate() {
    let request = ToolRequest::formal(
        AgentKind::AiKeeperOrchestrator,
        AgentTool::RequestSkillCheck,
    );
    let contract =
        trpg_test_support::authority_contract("campaign_b019_governance", AuthorityMode::AiKp, 1)
            .unwrap();
    let mut command =
        trpg_test_support::governed_command_for_contract(&contract, (), ActorRole::Workflow);
    command.write_path = FormalWritePath::DirectAgent;

    let error =
        adr_0009_agent_governance::validate_governed_tool_request(&contract, &command, &request)
            .unwrap_err();

    assert_eq!(error.code(), "AGENT_DIRECT_STATE_WRITE_FORBIDDEN");
}

#[test]
fn adr_0009_agent_governance_keeps_expression_agents_from_formal_tools() {
    let request = ToolRequest::formal(AgentKind::AtmosphereWriter, AgentTool::RevealClue);
    let contract =
        trpg_test_support::authority_contract("campaign_b019_governance", AuthorityMode::AiKp, 1)
            .unwrap();
    let command =
        trpg_test_support::governed_command_for_contract(&contract, (), ActorRole::Workflow);

    let error =
        adr_0009_agent_governance::validate_governed_tool_request(&contract, &command, &request)
            .unwrap_err();

    assert_eq!(error.code(), "TOOL_PERMISSION_DENIED");
}

#[test]
fn adr_0009_agent_governance_downgrades_human_kp_ai_formal_tool_to_draft() {
    let request = ToolRequest::formal(AgentKind::KeeperCopilot, AgentTool::ApplySanLoss);
    let contract = trpg_test_support::authority_contract(
        "campaign_b019_human_governance",
        AuthorityMode::HumanKp,
        1,
    )
    .unwrap();
    let command =
        trpg_test_support::governed_command_for_contract(&contract, (), ActorRole::Workflow);

    let decision =
        adr_0009_agent_governance::validate_governed_tool_request(&contract, &command, &request)
            .unwrap();

    assert!(!decision.tool_executed);
    assert!(decision.requires_human_confirmation);
    assert!(decision.draft_only);
    assert_eq!(decision.downgraded_to, Some(AgentTool::DraftSanLoss));
}
