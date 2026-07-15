use trpg_agent_runtime::rag_snapshot::RagChunk;
use trpg_agent_runtime::rag_snapshot_impl;
use trpg_agent_runtime::{PrincipalScope, Visibility, VisibilityLabel};

fn chunks() -> Vec<RagChunk> {
    vec![
        RagChunk::new(
            "rag_public_b019",
            "ruleset_pack",
            Visibility::new(VisibilityLabel::Public),
            "coc7-pack-0.1.0",
            "internal_gameplay",
        )
        .unwrap(),
        RagChunk::new(
            "rag_keeper_b019",
            "scenario",
            Visibility::new(VisibilityLabel::KeeperOnly),
            "golden-salt-bell-0.1.0",
            "campaign_only",
        )
        .unwrap(),
    ]
}

#[test]
fn rag_snapshot_impl_requires_embedding_model_and_metadata() {
    assert_eq!(
        trpg_test_support::normalized_prompt_id("trpg-agent-runtime", "rag_snapshot_impl"),
        "CODEX-0485-04-AI-AGENT-SYSTEM-962b774429"
    );
    let error =
        rag_snapshot_impl::RagSnapshotImpl::new("snapshot_bad", "", 2, chunks()).unwrap_err();

    assert_eq!(error.code(), "INVALID_CONFIGURATION");
}

#[test]
fn rag_snapshot_impl_filters_keeper_only_chunks_for_public_principal() {
    let snapshot =
        rag_snapshot_impl::RagSnapshotImpl::new("snapshot_b019", "embedding-local", 2, chunks())
            .unwrap();

    assert_eq!(snapshot.visible_chunks(&PrincipalScope::Public).len(), 1);
    assert_eq!(snapshot.visible_chunks(&PrincipalScope::Keeper).len(), 2);
    assert!(snapshot.is_rebuildable_from_event_count(2));
    assert!(!snapshot.is_rebuildable_from_event_count(1));
}
