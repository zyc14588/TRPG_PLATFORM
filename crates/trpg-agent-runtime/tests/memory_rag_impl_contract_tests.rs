pub mod common;

use trpg_agent_runtime::memory_rag_impl;
use trpg_agent_runtime::rag_snapshot::RagChunk;
use trpg_agent_runtime::{
    ActorRole, AgentDecision, AgentKind, AgentTool, ToolRequest, Visibility, VisibilityLabel,
};
use trpg_identity::{CampaignRole, GlobalRole, IdentityService, WorkloadRole};
use trpg_shared_kernel::EventStore;

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
fn memory_rag_impl_maps_batch_019_primary_contract() {
    assert_eq!(
        trpg_test_support::normalized_prompt_id("trpg-agent-runtime", "memory_rag_impl"),
        "CODEX-0483-04-AI-AGENT-SYSTEM-a577767984"
    );
    let modules = trpg_test_support::normalized_product_modules("trpg-agent-runtime");
    for module in [
        "agent_runtime::memory_rag_impl",
        "agent_runtime::model_provider_local_cloud_impl",
        "agent_runtime::rag_snapshot_impl",
        "agent_runtime::adr_0009_agent_governance",
    ] {
        assert!(modules.iter().any(|candidate| candidate == module));
    }
}

#[test]
fn memory_rag_impl_filters_context_chunks_and_replay_by_visibility() {
    let public_fact = common::context_fact(
        "fact_public_b019",
        "The safe fact is public.",
        Visibility::new(VisibilityLabel::Public),
    )
    .unwrap();
    let keeper_fact = common::context_fact(
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
    let authentication = trpg_test_support::ai_keeper_authentication("camp_ai_harbor");
    let decision = AgentDecision::new(
        "decision_memory_rag_b019",
        request,
        "check",
        &authentication,
    )
    .unwrap();
    let mut command = trpg_test_support::governed_command(
        decision.clone(),
        ActorRole::Workflow,
        trpg_agent_runtime::AuthorityMode::AiKp,
    );
    command.visibility = Visibility::new(VisibilityLabel::Public);
    store
        .append(
            &command,
            "MemoryRagSourceEvent",
            "memory_rag_source".to_owned(),
        )
        .unwrap();

    let identity = IdentityService::new(&[0x47; 32], 60_000).unwrap();
    let credential = identity
        .issue_workload_credential("agent_worker_b019", WorkloadRole::AgentWorker, 1, 20_000)
        .unwrap();
    let authentication = identity.authenticate_workload(&credential, 2).unwrap();
    let authorization = identity
        .verifier()
        .authorize_replay(
            &authentication,
            &trpg_shared_kernel::EntityId::new("camp_ai_harbor").unwrap(),
            3,
        )
        .unwrap();
    let view = memory_rag_impl::assemble_memory_rag_view(
        &[public_fact.clone(), keeper_fact],
        &chunks(),
        &store,
        &authorization,
        &authorization,
        4,
    )
    .unwrap();

    assert_eq!(view.context.facts.len(), 2);
    assert_eq!(view.context.facts[0], public_fact);
    assert_eq!(view.chunks.len(), 2);
    assert_eq!(view.visible_event_count, 1);
    assert!(memory_rag_impl::memory_rag_chunks_are_rebuildable(&chunks()));

    let mut user_identity = IdentityService::new(&[0x48; 32], 60_000).unwrap();
    user_identity
        .create_user(
            "owner_memory_rag_b019",
            "owner-memory-rag@example.test",
            "memory rag owner password",
            GlobalRole::ServerOwner,
        )
        .unwrap();
    user_identity
        .create_user(
            "player_memory_rag_b019",
            "player-memory-rag@example.test",
            "memory rag player password",
            GlobalRole::User,
        )
        .unwrap();
    let owner_session = user_identity
        .login(
            "owner-memory-rag@example.test",
            "memory rag owner password",
            1,
        )
        .unwrap();
    let owner = user_identity
        .authenticate_session(Some(owner_session.token.expose()), 2)
        .unwrap();
    user_identity
        .grant_membership(
            &owner,
            "camp_ai_harbor",
            "player_memory_rag_b019",
            CampaignRole::Player,
            3,
        )
        .unwrap();
    let player_session = user_identity
        .login(
            "player-memory-rag@example.test",
            "memory rag player password",
            1,
        )
        .unwrap();
    let player = user_identity
        .authenticate_session(Some(player_session.token.expose()), 2)
        .unwrap();
    let player_processor = user_identity
        .verifier()
        .authorize_replay(
            &player,
            &trpg_shared_kernel::EntityId::new("camp_ai_harbor").unwrap(),
            3,
        )
        .unwrap();
    let filtered = memory_rag_impl::assemble_memory_rag_view(
        &[
            public_fact,
            common::context_fact(
                "fact_keeper_processor_b019",
                "processor must not receive this",
                Visibility::new(VisibilityLabel::KeeperOnly),
            )
            .unwrap(),
        ],
        &chunks(),
        &store,
        &player_processor,
        &authorization,
        4,
    )
    .unwrap();
    assert_eq!(filtered.context.facts.len(), 1);
    assert_eq!(filtered.chunks.len(), 1);
}
