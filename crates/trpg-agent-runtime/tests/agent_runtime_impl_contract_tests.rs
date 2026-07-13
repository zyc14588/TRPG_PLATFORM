use trpg_agent_runtime::agent_runtime_impl;
use trpg_agent_runtime::{
    ActorRole, AgentDecision, AgentKind, AgentTool, AuthorityContract, AuthorityMode,
    CommandEnvelope, ContextFact, EventStore, FormalWritePath, PrincipalScope, ToolRequest,
    Visibility, VisibilityLabel,
};

fn ai_kp_command(payload: AgentDecision) -> CommandEnvelope<AgentDecision> {
    trpg_test_support::governed_command!(payload, ActorRole::Workflow, AuthorityMode::AiKp)
}

#[test]
fn agent_runtime_impl_rejects_direct_agent_state_write() {
    let boundary = agent_runtime_impl::agent_runtime_impl_boundary();
    assert!(boundary.event_store_boundary.contains("Event Store"));

    let request = ToolRequest::formal(
        AgentKind::AiKeeperOrchestrator,
        AgentTool::RequestSkillCheck,
    );
    let decision =
        AgentDecision::new("decision_b018_runtime_impl", request, "Library Use").unwrap();
    let mut command = ai_kp_command(decision.clone());
    command.write_path = FormalWritePath::DirectAgent;
    let contract =
        AuthorityContract::new("campaign_b018_runtime_impl", AuthorityMode::AiKp, 1).unwrap();
    let mut store = EventStore::default();

    let error =
        agent_runtime_impl::run_agent_runtime_decision(&mut store, &contract, &command, decision)
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
