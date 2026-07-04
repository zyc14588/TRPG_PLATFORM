use trpg_agent_runtime::adr_0009_agent_governance;
use trpg_agent_runtime::{
    ActorRole, AgentKind, AgentTool, AuthorityContract, AuthorityMode, CommandEnvelope,
    FormalWritePath, ToolRequest,
};

#[test]
fn adr_0009_agent_governance_keeps_gateway_and_default_deny() {
    assert_eq!(
        adr_0009_agent_governance::PROMPT_ID,
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
    let mut command = CommandEnvelope::governed((), ActorRole::Workflow, AuthorityMode::AiKp);
    command.write_path = FormalWritePath::DirectAgent;
    let contract =
        AuthorityContract::new("campaign_b019_governance", AuthorityMode::AiKp, 1).unwrap();

    let error =
        adr_0009_agent_governance::validate_governed_tool_request(&contract, &command, &request)
            .unwrap_err();

    assert_eq!(error.code(), "AGENT_DIRECT_STATE_WRITE_FORBIDDEN");
}

#[test]
fn adr_0009_agent_governance_keeps_expression_agents_from_formal_tools() {
    let request = ToolRequest::formal(AgentKind::AtmosphereWriter, AgentTool::RevealClue);
    let command = CommandEnvelope::governed((), ActorRole::Workflow, AuthorityMode::AiKp);
    let contract =
        AuthorityContract::new("campaign_b019_governance", AuthorityMode::AiKp, 1).unwrap();

    let error =
        adr_0009_agent_governance::validate_governed_tool_request(&contract, &command, &request)
            .unwrap_err();

    assert_eq!(error.code(), "ToolPermissionDenied");
}

#[test]
fn adr_0009_agent_governance_downgrades_human_kp_ai_formal_tool_to_draft() {
    let request = ToolRequest::formal(AgentKind::KeeperCopilot, AgentTool::ApplySanLoss);
    let command = CommandEnvelope::governed((), ActorRole::Workflow, AuthorityMode::HumanKp);
    let contract =
        AuthorityContract::new("campaign_b019_human_governance", AuthorityMode::HumanKp, 1)
            .unwrap();

    let decision =
        adr_0009_agent_governance::validate_governed_tool_request(&contract, &command, &request)
            .unwrap();

    assert!(!decision.tool_executed);
    assert!(decision.requires_human_confirmation);
    assert!(decision.draft_only);
    assert_eq!(decision.downgraded_to, Some(AgentTool::DraftSanLoss));
}
