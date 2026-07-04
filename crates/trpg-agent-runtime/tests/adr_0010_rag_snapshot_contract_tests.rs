use trpg_agent_runtime::adr_0010_rag_snapshot::{self, FrozenRagSnapshot};
use trpg_agent_runtime::{PrincipalScope, RagChunk, Visibility, VisibilityLabel};

fn chunks() -> Vec<RagChunk> {
    vec![
        RagChunk::new(
            "b020_public_rules",
            "ruleset_pack",
            Visibility::new(VisibilityLabel::Public),
            "coc7-pack-0.1.0",
            "internal_gameplay",
        )
        .unwrap(),
        RagChunk::new(
            "b020_keeper_truth",
            "scenario",
            Visibility::new(VisibilityLabel::KeeperOnly),
            "golden-salt-bell-0.1.0",
            "campaign_only",
        )
        .unwrap(),
    ]
}

#[test]
fn adr_0010_rag_snapshot_freezes_source_events_and_metadata() {
    assert_eq!(
        adr_0010_rag_snapshot::PROMPT_ID,
        "CODEX-0508-04-AI-AGENT-SYSTEM-f2ee9f2b79"
    );
    let snapshot = FrozenRagSnapshot::new("snapshot_b020", "embedding-local", 2, chunks()).unwrap();

    assert_eq!(snapshot.source_event_count, 2);
    assert!(snapshot.can_rebuild_from_event_count(2));
    assert!(!snapshot.can_rebuild_from_event_count(1));
}

#[test]
fn adr_0010_rag_snapshot_keeps_keeper_chunks_out_of_player_rag() {
    let snapshot =
        FrozenRagSnapshot::new("snapshot_b020_visibility", "embedding-local", 2, chunks()).unwrap();

    assert_eq!(snapshot.visible_chunks(&PrincipalScope::Public).len(), 1);
    assert_eq!(snapshot.visible_chunks(&PrincipalScope::Keeper).len(), 2);
}

#[test]
fn adr_0010_rag_snapshot_rejects_unfrozen_inputs() {
    let error = FrozenRagSnapshot::new("snapshot_bad", "", 0, chunks()).unwrap_err();

    assert_eq!(error.code(), "INVALID_CONFIGURATION");
}
