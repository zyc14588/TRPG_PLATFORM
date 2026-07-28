{
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
