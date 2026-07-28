
use trpg_data_eventing::postgre_sql_sq_lx_pgvector::{
    PgvectorRagError, PostgresRagSnapshotRepository, RagVisibleSearch,
};
use trpg_data_eventing::rag_snapshot::RagSnapshotChunkDraft;
use trpg_identity::{CampaignRole, GlobalRole, IdentityService, ReplayAuthorization};

use support::{draft, P04PostgresHarness};

fn rag_authorizations(
    campaign_id: &str,
    group_id: &str,
) -> (
    ReplayAuthorization,
    ReplayAuthorization,
    ReplayAuthorization,
    ReplayAuthorization,
) {
    let mut identity = IdentityService::new(&[0x96; 32], 60_000).unwrap();
    for (user_id, login, role) in [
        (
            "rag_keeper",
            "rag-keeper@example.test",
            GlobalRole::ServerOwner,
        ),
        ("rag_player", "rag-player@example.test", GlobalRole::User),
        (
            "rag_group_player",
            "rag-group-player@example.test",
            GlobalRole::User,
        ),
        (
            "rag_spectator",
            "rag-spectator@example.test",
            GlobalRole::User,
        ),
    ] {
        identity
            .create_user(user_id, login, "p04 rag password long enough", role)
            .unwrap();
    }
    let keeper_session = identity
        .login(
            "rag-keeper@example.test",
            "p04 rag password long enough",
            10_000,
        )
        .unwrap();
    let keeper = identity
        .authenticate_session(Some(keeper_session.token.expose()), 10_001)
        .unwrap();
    for (user_id, role) in [
        ("rag_keeper", CampaignRole::HumanKeeper),
        ("rag_player", CampaignRole::Player),
        ("rag_group_player", CampaignRole::Player),
        ("rag_spectator", CampaignRole::Spectator),
    ] {
        identity
            .grant_membership(&keeper, campaign_id, user_id, role, 10_002)
            .unwrap();
    }
    identity
        .create_campaign_group(&keeper, campaign_id, group_id, 10_002)
        .unwrap();
    identity
        .grant_group_membership(&keeper, campaign_id, group_id, "rag_group_player", 10_002)
        .unwrap();

    let mut authorize = |login: &str| {
        let session = identity
            .login(login, "p04 rag password long enough", 10_000)
            .unwrap();
        let authentication = identity
            .authenticate_session(Some(session.token.expose()), 10_001)
            .unwrap();
        identity
            .verifier()
            .authorize_replay(
                &authentication,
                &trpg_shared_kernel::EntityId::new(campaign_id).unwrap(),
                10_002,
            )
            .unwrap()
    };
    (
        authorize("rag-player@example.test"),
        authorize("rag-group-player@example.test"),
        authorize("rag-spectator@example.test"),
        identity
            .verifier()
            .authorize_replay(
                &keeper,
                &trpg_shared_kernel::EntityId::new(campaign_id).unwrap(),
                10_002,
            )
            .unwrap(),
    )
}

fn chunk(
    chunk_id: &str,
    source_event_sequence: i64,
    source_type: &str,
    content: &str,
    embedding: &[f32],
) -> RagSnapshotChunkDraft {
    RagSnapshotChunkDraft {
        chunk_id: chunk_id.to_owned(),
        source_event_sequence,
        derivation_event_sequence: source_event_sequence,
        source_type: source_type.to_owned(),
        copyright_status: "original".to_owned(),
        allowed_use: "campaign_only".to_owned(),
        content: content.to_owned(),
        embedding_model: "p04-local-embedding".to_owned(),
        embedding: embedding.to_vec(),
    }
}

async fn bind_formal_derivations(
    harness: &P04PostgresHarness,
    snapshot_id: &str,
    chunks: &mut [RagSnapshotChunkDraft],
) {
    for chunk in chunks {
        let source = sqlx::query(
            "SELECT campaign_id, stream_id, visibility_label, visibility_subject, \
             authority_owner, fact_provenance_kind, fact_provenance_reference, \
             fact_recorded_by FROM event_store WHERE sequence = $1",
        )
        .bind(chunk.source_event_sequence)
        .fetch_one(&harness.primary)
        .await
        .unwrap();
        let campaign_id: String = sqlx::Row::get(&source, "campaign_id");
        let stream_id: String = sqlx::Row::get(&source, "stream_id");
        let expected_version: i64 = sqlx::query_scalar(
            "SELECT max(stream_version) FROM event_store \
             WHERE campaign_id = $1 AND stream_id = $2",
        )
        .bind(&campaign_id)
        .bind(&stream_id)
        .fetch_one(&harness.primary)
        .await
        .unwrap();
        let commit_id = format!(
            "rag_derivation_{}_{}_{}",
            snapshot_id, chunk.chunk_id, expected_version
        );
        let mut derivation = draft(
            &campaign_id,
            &stream_id,
            &commit_id,
            expected_version,
            &["RagChunkDerived"],
        );
        derivation.visibility_label = sqlx::Row::get(&source, "visibility_label");
        derivation.visibility_subject = sqlx::Row::get(&source, "visibility_subject");
        derivation.authority_owner = sqlx::Row::get(&source, "authority_owner");
        derivation.provenance_kind = sqlx::Row::get(&source, "fact_provenance_kind");
        derivation.provenance_reference = sqlx::Row::get(&source, "fact_provenance_reference");
        derivation.provenance_recorded_by = sqlx::Row::get(&source, "fact_recorded_by");
        derivation.events[0].payload_json = serde_json::json!({
            "source_event_sequence": chunk.source_event_sequence,
            "snapshot_id": snapshot_id,
            "chunk_id": chunk.chunk_id,
            "content_hash": chunk.content_hash(),
            "source_type": chunk.source_type,
            "copyright_status": chunk.copyright_status,
            "allowed_use": chunk.allowed_use,
            "embedding_model": chunk.embedding_model,
            "embedding_dimensions": chunk.embedding.len(),
            "embedding_hash": chunk.embedding_hash(),
        })
        .to_string();
        let receipt = harness.store.commit(&derivation).await.unwrap();
        chunk.derivation_event_sequence = receipt.first_event_sequence;
    }
}
