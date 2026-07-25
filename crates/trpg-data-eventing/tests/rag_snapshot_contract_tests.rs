mod support;

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

#[tokio::test]
async fn pgvector_snapshot_is_rebuildable_and_filters_visibility_before_materialization() {
    let harness = P04PostgresHarness::reset().await;
    let campaign_id = "p04_rag_campaign";
    let stream_id = "p04_rag_scene";
    let snapshot_id = "p04_rag_snapshot";

    let mut public_event = draft(
        campaign_id,
        stream_id,
        "p04_rag_public",
        0,
        &["PublicClueRecorded"],
    );
    public_event.visibility_label = "public".to_owned();
    public_event.visibility_subject = "not_applicable".to_owned();
    let public_commit = harness.store.commit(&public_event).await.unwrap();

    let mut keeper_event = draft(
        campaign_id,
        stream_id,
        "p04_rag_keeper",
        1,
        &["KeeperTruthRecorded"],
    );
    keeper_event.visibility_label = "keeper_only".to_owned();
    keeper_event.visibility_subject = "not_applicable".to_owned();
    let keeper_commit = harness.store.commit(&keeper_event).await.unwrap();

    let party_event = draft(
        campaign_id,
        stream_id,
        "p04_rag_party",
        2,
        &["PartyClueRecorded"],
    );
    let party_commit = harness.store.commit(&party_event).await.unwrap();

    let mut group_event = draft(
        campaign_id,
        stream_id,
        "p04_rag_group",
        3,
        &["GroupSecretRecorded"],
    );
    group_event.visibility_label = "private_to_group".to_owned();
    group_event.visibility_subject = "group_a".to_owned();
    let group_commit = harness.store.commit(&group_event).await.unwrap();

    let mut spectator_visible_event = draft(
        campaign_id,
        stream_id,
        "p04_rag_spectator_visible",
        4,
        &["SpectatorUpdateRecorded"],
    );
    spectator_visible_event.visibility_label = "spectator_visible".to_owned();
    spectator_visible_event.visibility_subject = "not_applicable".to_owned();
    let spectator_visible_commit = harness
        .store
        .commit(&spectator_visible_event)
        .await
        .unwrap();

    let mut spectator_hidden_event = draft(
        campaign_id,
        stream_id,
        "p04_rag_spectator_hidden",
        5,
        &["PlayerOnlyUpdateRecorded"],
    );
    spectator_hidden_event.visibility_label = "spectator_hidden".to_owned();
    spectator_hidden_event.visibility_subject = "not_applicable".to_owned();
    let spectator_hidden_commit = harness.store.commit(&spectator_hidden_event).await.unwrap();

    let canonical_count_before: i64 = sqlx::query_scalar("SELECT count(*) FROM event_store")
        .fetch_one(&harness.primary)
        .await
        .unwrap();
    let mut chunks = vec![
        chunk(
            "public_harbor_clue",
            public_commit.first_event_sequence,
            "campaign_memory",
            "The harbor ledger is visible to everyone.",
            &[1.0, 0.0, 0.0],
        ),
        chunk(
            "keeper_cult_truth",
            keeper_commit.first_event_sequence,
            "scenario_truth",
            "The lighthouse keeper leads the hidden cult.",
            &[0.0, 1.0, 0.0],
        ),
        chunk(
            "party_tide_table",
            party_commit.first_event_sequence,
            "campaign_memory",
            "The full party can inspect the tide table.",
            &[0.8, 0.2, 0.0],
        ),
        chunk(
            "group_cipher",
            group_commit.first_event_sequence,
            "campaign_memory",
            "Only group A knows the cipher phrase.",
            &[0.7, 0.3, 0.0],
        ),
        chunk(
            "spectator_scoreboard",
            spectator_visible_commit.first_event_sequence,
            "campaign_memory",
            "Spectators may see the public-facing scoreboard.",
            &[0.6, 0.4, 0.0],
        ),
        chunk(
            "hidden_tactical_note",
            spectator_hidden_commit.first_event_sequence,
            "campaign_memory",
            "Active players may see this tactical note.",
            &[0.5, 0.5, 0.0],
        ),
    ];
    bind_formal_derivations(&harness, snapshot_id, &mut chunks).await;
    let repository = PostgresRagSnapshotRepository::new(harness.primary.clone());
    assert_eq!(
        repository
            .replace_snapshot(campaign_id, snapshot_id, &chunks)
            .await
            .unwrap(),
        6
    );
    let (player_authorization, group_authorization, spectator_authorization, keeper_authorization) =
        rag_authorizations(campaign_id, "group_a");

    let public_hits = repository
        .search_visible(RagVisibleSearch {
            campaign_id,
            snapshot_id,
            authorization: None,
            now_unix_ms: 10_003,
            embedding_model: "p04-local-embedding",
            embedding: &[0.0, 1.0, 0.0],
            limit: 10,
        })
        .await
        .unwrap();
    assert_eq!(public_hits.len(), 1);
    assert_eq!(public_hits[0].chunk.chunk_id(), "public_harbor_clue");
    assert_eq!(public_hits[0].chunk.visibility(), "public");

    let player_hits = repository
        .search_visible(RagVisibleSearch {
            campaign_id,
            snapshot_id,
            authorization: Some(&player_authorization),
            now_unix_ms: 10_003,
            embedding_model: "p04-local-embedding",
            embedding: &[1.0, 0.0, 0.0],
            limit: 10,
        })
        .await
        .unwrap();
    assert_eq!(player_hits.len(), 4);
    assert!(player_hits
        .iter()
        .any(|hit| hit.chunk.chunk_id() == "party_tide_table"));
    assert!(player_hits
        .iter()
        .any(|hit| hit.chunk.chunk_id() == "hidden_tactical_note"));
    assert!(!player_hits
        .iter()
        .any(|hit| hit.chunk.chunk_id() == "group_cipher"));

    let group_hits = repository
        .search_visible(RagVisibleSearch {
            campaign_id,
            snapshot_id,
            authorization: Some(&group_authorization),
            now_unix_ms: 10_003,
            embedding_model: "p04-local-embedding",
            embedding: &[1.0, 0.0, 0.0],
            limit: 10,
        })
        .await
        .unwrap();
    assert_eq!(group_hits.len(), 5);
    assert!(group_hits
        .iter()
        .any(|hit| hit.chunk.chunk_id() == "group_cipher"));

    let spectator_hits = repository
        .search_visible(RagVisibleSearch {
            campaign_id,
            snapshot_id,
            authorization: Some(&spectator_authorization),
            now_unix_ms: 10_003,
            embedding_model: "p04-local-embedding",
            embedding: &[1.0, 0.0, 0.0],
            limit: 10,
        })
        .await
        .unwrap();
    assert_eq!(spectator_hits.len(), 2);
    assert!(spectator_hits
        .iter()
        .any(|hit| hit.chunk.chunk_id() == "spectator_scoreboard"));
    assert!(!spectator_hits
        .iter()
        .any(|hit| hit.chunk.chunk_id() == "hidden_tactical_note"));

    let keeper_hits = repository
        .search_visible(RagVisibleSearch {
            campaign_id,
            snapshot_id,
            authorization: Some(&keeper_authorization),
            now_unix_ms: 10_003,
            embedding_model: "p04-local-embedding",
            embedding: &[0.0, 1.0, 0.0],
            limit: 10,
        })
        .await
        .unwrap();
    assert_eq!(keeper_hits.len(), 6);
    assert_eq!(keeper_hits[0].chunk.chunk_id(), "keeper_cult_truth");
    assert!(keeper_hits[0].cosine_distance < keeper_hits[1].cosine_distance);
    assert_eq!(keeper_hits[0].chunk.version(), 2);
    assert_eq!(keeper_hits[0].chunk.owner(), keeper_event.authority_owner);
    assert_eq!(keeper_hits[0].chunk.copyright_status(), "original");
    assert_eq!(
        keeper_hits[0].chunk.fact_provenance().kind(),
        keeper_event.provenance_kind
    );
    assert_eq!(
        keeper_hits[0].chunk.fact_provenance().reference(),
        keeper_event.provenance_reference
    );
    assert_eq!(
        keeper_hits[0].chunk.fact_provenance().recorded_by(),
        keeper_event.provenance_recorded_by
    );

    // A direct projector write cannot downgrade Keeper-only source metadata to
    // public: the database independently checks it against Event Store.
    let mut forged_evidence = vec![chunk(
        "forged_public_chunk",
        keeper_commit.first_event_sequence,
        "scenario_truth",
        "forged public content",
        &[0.0, 1.0, 0.0],
    )];
    bind_formal_derivations(&harness, "p04_forged_snapshot", &mut forged_evidence).await;
    let forged = sqlx::query(
        r#"
        INSERT INTO rag_snapshot_chunk (
            campaign_id, snapshot_id, chunk_id, source_event_sequence,
            derivation_event_sequence,
            source_type, visibility, visibility_subject, copyright_status,
            version, owner, allowed_use, fact_provenance, chunk_hash, content,
            embedding_model, embedding_dimensions, embedding
        )
        SELECT event.campaign_id, 'p04_forged_snapshot', 'forged_public_chunk',
               event.sequence, $2, 'scenario_truth', 'public', 'not_applicable',
               'original', event.stream_version, event.authority_owner,
               'campaign_only',
               jsonb_build_object(
                   'kind', event.fact_provenance_kind,
                   'reference', event.fact_provenance_reference,
                   'recorded_by', event.fact_recorded_by
               ),
               'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
               'forged public content', 'p04-local-embedding', 3,
               '[0,1,0]'::vector
          FROM event_store AS event
         WHERE event.sequence = $1
        "#,
    )
    .bind(keeper_commit.first_event_sequence)
    .bind(forged_evidence[0].derivation_event_sequence)
    .execute(&harness.primary)
    .await;
    assert!(
        forged.is_err(),
        "Keeper-only RAG metadata downgrade was accepted"
    );

    // Required copyright metadata is enforced independently by PostgreSQL;
    // a raw projector cannot bypass the Rust validation layer.
    let mut copyright_evidence = vec![chunk(
        "invalid_copyright_chunk",
        public_commit.first_event_sequence,
        "campaign_memory",
        "copyright probe",
        &[1.0, 0.0, 0.0],
    )];
    bind_formal_derivations(
        &harness,
        "p04_invalid_copyright_snapshot",
        &mut copyright_evidence,
    )
    .await;
    let invalid_database_copyright = sqlx::query(
        r#"
        INSERT INTO rag_snapshot_chunk (
            campaign_id, snapshot_id, chunk_id, source_event_sequence,
            derivation_event_sequence,
            source_type, visibility, visibility_subject, copyright_status,
            version, owner, allowed_use, fact_provenance, chunk_hash, content,
            embedding_model, embedding_dimensions, embedding
        )
        SELECT event.campaign_id, 'p04_invalid_copyright_snapshot',
               'invalid_copyright_chunk', event.sequence, $2, 'campaign_memory',
               event.visibility_label, event.visibility_subject, 'UNDECLARED',
               event.stream_version, event.authority_owner, 'campaign_only',
               jsonb_build_object(
                   'kind', event.fact_provenance_kind,
                   'reference', event.fact_provenance_reference,
                   'recorded_by', event.fact_recorded_by
               ),
               encode(sha256(convert_to('copyright probe', 'UTF8')), 'hex'),
               'copyright probe', 'p04-local-embedding', 3,
               '[1,0,0]'::vector
          FROM event_store AS event
         WHERE event.sequence = $1
        "#,
    )
    .bind(public_commit.first_event_sequence)
    .bind(copyright_evidence[0].derivation_event_sequence)
    .execute(&harness.primary)
    .await;
    assert!(
        invalid_database_copyright.is_err(),
        "database accepted an invalid RAG copyright status"
    );

    let invalid_copyright = chunks[0].clone();
    let mut invalid_copyright = invalid_copyright;
    invalid_copyright.chunk_id = "invalid_copyright".to_owned();
    invalid_copyright.copyright_status = "".to_owned();
    assert!(matches!(
        repository
            .replace_snapshot(
                campaign_id,
                "invalid_copyright_snapshot",
                &[invalid_copyright]
            )
            .await,
        Err(PgvectorRagError::Validation(_))
    ));

    let first_hashes: Vec<String> = sqlx::query_scalar(
        "SELECT chunk_hash FROM rag_snapshot_chunk WHERE campaign_id = $1 AND snapshot_id = $2 ORDER BY chunk_id",
    )
    .bind(campaign_id)
    .bind(snapshot_id)
    .fetch_all(&harness.primary)
    .await
    .unwrap();
    repository
        .replace_snapshot(campaign_id, snapshot_id, &chunks)
        .await
        .unwrap();
    let rebuilt_hashes: Vec<String> = sqlx::query_scalar(
        "SELECT chunk_hash FROM rag_snapshot_chunk WHERE campaign_id = $1 AND snapshot_id = $2 ORDER BY chunk_id",
    )
    .bind(campaign_id)
    .bind(snapshot_id)
    .fetch_all(&harness.primary)
    .await
    .unwrap();
    assert_eq!(rebuilt_hashes, first_hashes);

    let invalid_rebuild = repository
        .replace_snapshot(
            campaign_id,
            snapshot_id,
            &[chunk(
                "missing_source",
                9_999_999,
                "campaign_memory",
                "This source event does not exist.",
                &[1.0, 0.0, 0.0],
            )],
        )
        .await;
    assert_eq!(
        invalid_rebuild,
        Err(PgvectorRagError::SourceEventNotFound {
            sequence: 9_999_999
        })
    );
    let retained_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM rag_snapshot_chunk WHERE campaign_id = $1 AND snapshot_id = $2",
    )
    .bind(campaign_id)
    .bind(snapshot_id)
    .fetch_one(&harness.primary)
    .await
    .unwrap();
    assert_eq!(
        retained_count, 6,
        "failed rebuild must roll back its delete"
    );

    // A transaction-scoped snapshot lock makes the embedding contract safe
    // even for raw concurrent writers that bypass this repository. Before the
    // P04 repair both inserts committed and produced a mixed-model snapshot.
    let raw_snapshot_id = "p04_rag_concurrent_contract";
    let mut raw_evidence = vec![
        chunk(
            "raw_chunk_a",
            public_commit.first_event_sequence,
            "campaign_memory",
            "raw content a",
            &[1.0, 0.0, 0.0],
        ),
        chunk(
            "raw_chunk_b",
            keeper_commit.first_event_sequence,
            "campaign_memory",
            "raw content b",
            &[0.0, 1.0, 0.0, 0.0],
        ),
    ];
    raw_evidence[0].embedding_model = "raw_model_a".to_owned();
    raw_evidence[1].embedding_model = "raw_model_b".to_owned();
    bind_formal_derivations(&harness, raw_snapshot_id, &mut raw_evidence).await;
    let first_raw = insert_raw_snapshot_chunk(
        &harness.primary,
        RawSnapshotChunkInsert {
            campaign_id,
            snapshot_id: raw_snapshot_id,
            embedding_model: "raw_model_a",
            source_event_sequence: public_commit.first_event_sequence,
            derivation_event_sequence: raw_evidence[0].derivation_event_sequence,
            chunk_id: "raw_chunk_a",
            content: "raw content a",
            embedding_dimensions: 3,
            embedding: "[1,0,0]",
        },
    );
    let second_raw = insert_raw_snapshot_chunk(
        &harness.primary,
        RawSnapshotChunkInsert {
            campaign_id,
            snapshot_id: raw_snapshot_id,
            embedding_model: "raw_model_b",
            source_event_sequence: keeper_commit.first_event_sequence,
            derivation_event_sequence: raw_evidence[1].derivation_event_sequence,
            chunk_id: "raw_chunk_b",
            content: "raw content b",
            embedding_dimensions: 4,
            embedding: "[0,1,0,0]",
        },
    );
    let (first_raw, second_raw) = tokio::join!(first_raw, second_raw);
    assert_eq!(
        [first_raw.is_ok(), second_raw.is_ok()]
            .into_iter()
            .filter(|succeeded| *succeeded)
            .count(),
        1,
        "exactly one incompatible concurrent embedding contract may commit"
    );
    let raw_contracts: Vec<(String, i32)> = sqlx::query_as(
        "SELECT embedding_model, embedding_dimensions FROM rag_snapshot_chunk WHERE campaign_id = $1 AND snapshot_id = $2",
    )
    .bind(campaign_id)
    .bind(raw_snapshot_id)
    .fetch_all(&harness.primary)
    .await
    .unwrap();
    assert_eq!(raw_contracts.len(), 1);

    // Legitimate repository replacements use the same lock before DELETE,
    // so concurrent rebuild requests serialize and both complete without
    // exposing or retaining a mixed generation.
    let replacement_snapshot_id = "p04_rag_concurrent_replace";
    let mut replacement_a = vec![chunk(
        "replacement_a",
        public_commit.first_event_sequence,
        "campaign_memory",
        "replacement generation a",
        &[1.0, 0.0, 0.0],
    )];
    let mut replacement_b_chunk = chunk(
        "replacement_b",
        keeper_commit.first_event_sequence,
        "campaign_memory",
        "replacement generation b",
        &[0.0, 1.0, 0.0, 0.0],
    );
    replacement_b_chunk.embedding_model = "p04-local-embedding-v2".to_owned();
    let mut replacement_b = vec![replacement_b_chunk];
    bind_formal_derivations(&harness, replacement_snapshot_id, &mut replacement_a).await;
    bind_formal_derivations(&harness, replacement_snapshot_id, &mut replacement_b).await;
    let first_repository = repository.clone();
    let second_repository = repository.clone();
    let (replacement_a_result, replacement_b_result) = tokio::join!(
        first_repository.replace_snapshot(campaign_id, replacement_snapshot_id, &replacement_a),
        second_repository.replace_snapshot(campaign_id, replacement_snapshot_id, &replacement_b),
    );
    assert_eq!(replacement_a_result.unwrap(), 1);
    assert_eq!(replacement_b_result.unwrap(), 1);
    let final_replacement: Vec<(String, String, i32)> = sqlx::query_as(
        "SELECT chunk_id, embedding_model, embedding_dimensions FROM rag_snapshot_chunk WHERE campaign_id = $1 AND snapshot_id = $2",
    )
    .bind(campaign_id)
    .bind(replacement_snapshot_id)
    .fetch_all(&harness.primary)
    .await
    .unwrap();
    assert_eq!(final_replacement.len(), 1);
    assert!(
        matches!(
            final_replacement[0],
            (ref chunk_id, ref model, 3)
                if chunk_id == "replacement_a" && model == "p04-local-embedding"
        ) || matches!(
            final_replacement[0],
            (ref chunk_id, ref model, 4)
                if chunk_id == "replacement_b" && model == "p04-local-embedding-v2"
        )
    );

    let canonical_source_count_after: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM event_store WHERE event_type <> 'RagChunkDerived'",
    )
    .fetch_one(&harness.primary)
    .await
    .unwrap();
    assert_eq!(canonical_source_count_after, canonical_count_before);
    let derivation_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM event_store WHERE event_type = 'RagChunkDerived'")
            .fetch_one(&harness.primary)
            .await
            .unwrap();
    assert!(derivation_count >= 12);
    harness.store.verify_integrity().await.unwrap();
}

struct RawSnapshotChunkInsert<'a> {
    campaign_id: &'a str,
    snapshot_id: &'a str,
    embedding_model: &'a str,
    source_event_sequence: i64,
    derivation_event_sequence: i64,
    chunk_id: &'a str,
    content: &'a str,
    embedding_dimensions: i32,
    embedding: &'a str,
}

async fn insert_raw_snapshot_chunk(
    pool: &sqlx::PgPool,
    insert: RawSnapshotChunkInsert<'_>,
) -> Result<(), sqlx::Error> {
    let mut transaction = pool.begin().await?;
    sqlx::query(
        r#"
        INSERT INTO rag_snapshot_chunk (
            campaign_id, snapshot_id, chunk_id, source_event_sequence,
            derivation_event_sequence,
            source_type, visibility, visibility_subject, copyright_status,
            version, owner, allowed_use, fact_provenance, chunk_hash, content,
            embedding_model, embedding_dimensions, embedding
        )
        SELECT event.campaign_id, $2, $3, event.sequence, $9,
               'campaign_memory', event.visibility_label,
               event.visibility_subject, 'original', event.stream_version,
               event.authority_owner, 'campaign_only',
               jsonb_build_object(
                   'kind', event.fact_provenance_kind,
                   'reference', event.fact_provenance_reference,
                   'recorded_by', event.fact_recorded_by
               ),
               encode(sha256(convert_to($4, 'UTF8')), 'hex'), $4,
               $5, $6, $7::vector
          FROM event_store AS event
         WHERE event.campaign_id = $1 AND event.sequence = $8
        "#,
    )
    .bind(insert.campaign_id)
    .bind(insert.snapshot_id)
    .bind(insert.chunk_id)
    .bind(insert.content)
    .bind(insert.embedding_model)
    .bind(insert.embedding_dimensions)
    .bind(insert.embedding)
    .bind(insert.source_event_sequence)
    .bind(insert.derivation_event_sequence)
    .execute(&mut *transaction)
    .await?;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    transaction.commit().await?;
    Ok(())
}
