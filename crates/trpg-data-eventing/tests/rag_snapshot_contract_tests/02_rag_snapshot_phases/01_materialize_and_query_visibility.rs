{
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
    include!("02_rebuild_integrity_and_expiry.rs");
}
