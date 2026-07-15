mod common;

use trpg_agent_runtime::agent_runtime_impl;
use trpg_agent_runtime::{
    ActorRole, AgentDecision, AgentDecisionCommitter, AgentKind, AgentTool, AuthorityMode,
    CommandEnvelope, ContextFact, FormalWritePath, PrincipalScope, ToolRequest, Visibility,
    VisibilityLabel,
};

fn ai_kp_command(payload: AgentDecision) -> CommandEnvelope<AgentDecision> {
    trpg_test_support::governed_command(payload, ActorRole::Workflow, AuthorityMode::AiKp)
}

#[test]
fn agent_runtime_impl_rejects_direct_agent_state_write() {
    assert_eq!(
        trpg_test_support::normalized_prompt_id("trpg-agent-runtime", "agent_runtime_impl"),
        "CODEX-0481-04-AI-AGENT-SYSTEM-b5f1e3af9c"
    );
    let boundary = agent_runtime_impl::agent_runtime_impl_boundary();
    assert!(boundary.event_store_boundary.contains("Event Store"));

    let request = ToolRequest::formal(
        AgentKind::AiKeeperOrchestrator,
        AgentTool::RequestSkillCheck,
    );
    let authentication = trpg_test_support::ai_keeper_authentication("camp_ai_harbor");
    let decision = AgentDecision::new(
        "decision_b018_runtime_impl",
        request,
        "Library Use",
        &authentication,
    )
    .unwrap();
    let mut command = ai_kp_command(decision.clone());
    command.write_path = FormalWritePath::DirectAgent;
    let contract =
        trpg_test_support::authority_contract("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    let committer =
        AgentDecisionCommitter::new(trpg_test_support::identity_verifier_for_contract(&contract))
            .unwrap();
    let mut store = common::audited_store();

    let error = agent_runtime_impl::run_agent_runtime_decision(
        &committer,
        &mut store,
        &command,
        &trpg_test_support::workflow_authentication(),
        decision,
        2,
    )
    .unwrap_err();

    assert_eq!(error.code(), "AGENT_DIRECT_STATE_WRITE_FORBIDDEN");
    assert!(store.events().is_empty());
}

#[test]
fn agent_runtime_impl_filters_context_by_visibility_label() {
    let public_fact = ContextFact::new(
        "fact_b018_public",
        "The front door is unlocked.",
        Visibility::new(VisibilityLabel::Public),
    )
    .unwrap();
    let keeper_fact = ContextFact::new(
        "fact_b018_keeper",
        "keeper_only ai_internal",
        Visibility::new(VisibilityLabel::KeeperOnly),
    )
    .unwrap();

    let public_context = agent_runtime_impl::assemble_runtime_context(
        &[public_fact.clone(), keeper_fact],
        &PrincipalScope::Public,
    );

    assert_eq!(public_context.facts, vec![public_fact]);
    assert_eq!(public_context.strictest_visibility, VisibilityLabel::Public);
}
