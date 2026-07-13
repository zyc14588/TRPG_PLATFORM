use trpg_agent_runtime::memory_rag_impl;
use trpg_agent_runtime::rag_snapshot::RagChunk;
use trpg_agent_runtime::{
    ActorRole, AgentDecision, AgentKind, AgentModule, AgentTool, ContextFact, EventStore,
    PrincipalScope, ToolRequest, Visibility, VisibilityLabel, AGENT_RUNTIME_MODULES,
};

fn chunks() -> Vec<RagChunk> {
    vec![
        RagChunk::new(
            "chunk_public_b019",
            "ruleset_pack",
            Visibility::new(VisibilityLabel::Public),
            "coc7-pack-0.1.0",
            "internal_gameplay",
        )
        .unwrap(),
        RagChunk::new(
            "chunk_keeper_b019",
            "scenario",
            Visibility::new(VisibilityLabel::KeeperOnly),
            "golden-salt-bell-0.1.0",
            "campaign_only",
        )
        .unwrap(),
    ]
}

#[test]
fn memory_and_rag_implementations_are_registered_by_product_role() {
    for module in [
        AgentModule::MemoryRagImpl,
        AgentModule::ModelProviderLocalCloudImpl,
        AgentModule::RagSnapshotImpl,
        AgentModule::Adr0009AgentGovernance,
    ] {
        assert!(AGENT_RUNTIME_MODULES.contains(&module));
    }
}

#[test]
fn memory_rag_impl_filters_context_chunks_and_replay_by_visibility() {
    let public_fact = ContextFact::new(
        "fact_public_b019",
        "The safe fact is public.",
        Visibility::new(VisibilityLabel::Public),
    )
    .unwrap();
    let keeper_fact = ContextFact::new(
        "fact_keeper_b019",
        "keeper_only ai_internal",
        Visibility::new(VisibilityLabel::KeeperOnly),
    )
    .unwrap();
    let mut store = EventStore::default();
    let request = ToolRequest::formal(
        AgentKind::AiKeeperOrchestrator,
        AgentTool::RequestSkillCheck,
    );
    let decision = AgentDecision::new("decision_memory_rag_b019", request, "check").unwrap();
    let mut command = trpg_test_support::governed_command!(
        decision.clone(),
        ActorRole::Workflow,
        trpg_agent_runtime::AuthorityMode::AiKp,
    );
    command.visibility = Visibility::new(VisibilityLabel::Public);
    store
        .append(&command, "MemoryRagSourceEvent", decision)
        .unwrap();

    let view = memory_rag_impl::assemble_memory_rag_view(
        &[public_fact.clone(), keeper_fact],
        &chunks(),
        &store,
        &PrincipalScope::Public,
    );

    assert_eq!(view.context.facts, vec![public_fact]);
    assert_eq!(view.chunks.len(), 1);
    assert_eq!(view.visible_event_count, 1);
    assert!(memory_rag_impl::memory_rag_chunks_are_rebuildable(&chunks()));
}
