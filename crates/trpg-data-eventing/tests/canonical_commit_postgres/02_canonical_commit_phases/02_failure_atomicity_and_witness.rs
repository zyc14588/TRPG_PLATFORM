{

    // Make only the witness finalization fail. The primary commit remains
    // durable; retrying the same idempotent request repairs the witness without
    // duplicating any event or audit row.
    sqlx::raw_sql(
        r#"
        CREATE OR REPLACE FUNCTION reject_witness_finalize_probe()
        RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN
            IF NEW.phase = 'COMMITTED' THEN
                RAISE EXCEPTION 'witness finalize probe rejection';
            END IF;
            RETURN NEW;
        END;
        $$;
        DROP TRIGGER IF EXISTS reject_witness_finalize_probe ON external_audit_witness;
        CREATE TRIGGER reject_witness_finalize_probe
        BEFORE INSERT ON external_audit_witness
        FOR EACH ROW EXECUTE FUNCTION reject_witness_finalize_probe();
        "#,
    )
    .execute(&witness)
    .await
    .unwrap();

    let pending_draft = draft("finalize_gap", 2, &["SceneAdvanced"]);
    let pending = store.commit(&pending_draft).await;
    assert!(matches!(
        pending,
        Err(CanonicalStoreError::WitnessFinalizationPending { .. })
    ));
    assert_eq!(
        scalar(&primary, "SELECT count(*) FROM event_store").await,
        event_count_before_rollback + 1
    );
    assert_eq!(
        scalar(&primary, "SELECT count(*) FROM event_outbox").await,
        outbox_count_before_rollback + 1
    );

    sqlx::raw_sql(
        r#"
        DROP TRIGGER IF EXISTS reject_witness_finalize_probe ON external_audit_witness;
        DROP FUNCTION IF EXISTS reject_witness_finalize_probe();
        "#,
    )
    .execute(&witness)
    .await
    .unwrap();

    let retry = store.commit(&pending_draft).await.unwrap();
    assert_eq!(retry.first_event_sequence, retry.last_event_sequence);
    assert_eq!(
        scalar(&primary, "SELECT count(*) FROM event_store").await,
        event_count_before_rollback + 1
    );
    assert_eq!(
        scalar(&primary, "SELECT count(*) FROM event_outbox").await,
        outbox_count_before_rollback + 1
    );
    assert_eq!(
        scalar(&primary, "SELECT count(*) FROM canonical_audit_log").await,
        audit_count_before_rollback + 1
    );
    assert_eq!(
        scalar(&primary, "SELECT count(*) FROM formal_commits").await,
        formal_count_before_rollback + 1
    );

    // The witness tables are append-only, including TRUNCATE protection.
    assert!(
        sqlx::query("DELETE FROM external_audit_witness WHERE commit_id = 'success'")
            .execute(&witness)
            .await
            .is_err()
    );
    assert!(sqlx::query("TRUNCATE external_audit_witness")
        .execute(&witness)
        .await
        .is_err());

    let mut mismatched_audit_scope = draft("mismatched_audit_scope", 0, &["Rejected"]);
    mismatched_audit_scope.campaign_id = "campaign_other".to_owned();
    assert!(matches!(
        store.commit(&mismatched_audit_scope).await,
        Err(CanonicalStoreError::Validation(
            "audit_campaign_resource_mismatch"
        ))
    ));

    // Idempotency and expected version are bound to campaign/stream/operation.
    // Two authorized resources in one campaign are independent streams even
    // when they reuse the same client idempotency key.
    let mut same_campaign_a = draft("same_campaign_a", 0, &["SceneAdvanced"]);
    bind_campaign(&mut same_campaign_a, "campaign_multi_stream");
    bind_stream(&mut same_campaign_a, "scene_alpha");
    same_campaign_a.idempotency_key = "shared_same_campaign_key".to_owned();
    let mut same_campaign_b = draft("same_campaign_b", 0, &["SceneAdvanced"]);
    bind_campaign(&mut same_campaign_b, "campaign_multi_stream");
    bind_stream(&mut same_campaign_b, "scene_beta");
    same_campaign_b.idempotency_key = "shared_same_campaign_key".to_owned();
    let same_campaign_a_result = store.commit(&same_campaign_a).await.unwrap();
    let same_campaign_b_result = store.commit(&same_campaign_b).await.unwrap();
    assert_eq!(same_campaign_a_result.first_stream_version, 1);
    assert_eq!(same_campaign_b_result.first_stream_version, 1);
    let stored_streams: Vec<(String, i64)> = sqlx::query_as(
        "SELECT stream_id, stream_version FROM event_store WHERE campaign_id = $1 ORDER BY stream_id",
    )
    .bind("campaign_multi_stream")
    .fetch_all(&primary)
    .await
    .unwrap();
    assert_eq!(
        stored_streams,
        vec![("scene_alpha".to_owned(), 1), ("scene_beta".to_owned(), 1)]
    );

    let mut mismatched_stream_scope = draft("mismatched_stream_scope", 0, &["Rejected"]);
    bind_campaign(&mut mismatched_stream_scope, "campaign_multi_stream");
    mismatched_stream_scope.stream_id = "scene_ungranted".to_owned();
    mismatched_stream_scope.audit.resource_type = "scene".to_owned();
    mismatched_stream_scope.audit.resource_id = "scene_granted".to_owned();
    assert!(matches!(
        store.commit(&mismatched_stream_scope).await,
        Err(CanonicalStoreError::Validation(
            "stream_audit_resource_mismatch"
        ))
    ));

    // Reusing a key in a different campaign is also valid; changing the
    // request inside one exact scope is rejected without another append.
    let mut scope_a = draft("scope_a", 0, &["CampaignScopedEvent"]);
    bind_campaign(&mut scope_a, "campaign_scope_a");
    scope_a.idempotency_key = "shared_scoped_key".to_owned();
    let mut scope_b = draft("scope_b", 0, &["CampaignScopedEvent"]);
    bind_campaign(&mut scope_b, "campaign_scope_b");
    scope_b.idempotency_key = "shared_scoped_key".to_owned();
    store.commit(&scope_a).await.unwrap();
    store.commit(&scope_b).await.unwrap();

    let mut conflicting = draft("scope_a_conflict", 1, &["DifferentRequest"]);
    bind_campaign(&mut conflicting, "campaign_scope_a");
    conflicting.idempotency_key = "shared_scoped_key".to_owned();
    assert!(matches!(
        store.commit(&conflicting).await,
        Err(CanonicalStoreError::IdempotencyConflict)
    ));
    assert_eq!(
        scalar(
            &primary,
            "SELECT count(*) FROM event_store WHERE campaign_id IN ('campaign_scope_a', 'campaign_scope_b')"
        )
        .await,
        2
    );

    // Global event sequences may interleave across independently locked
    // campaign streams. Hold two advisory barriers until both transactions
    // have allocated their first sequence, then release them together. This
    // deterministically overlaps the ranges without timing-based pg_sleep.
    let mut barrier = PgConnection::connect(&primary_url).await.unwrap();
    sqlx::query("SELECT pg_advisory_lock(90316001), pg_advisory_lock(90316002)")
        .execute(&mut barrier)
        .await
        .unwrap();
    let sequence_before: i64 =
        sqlx::query_scalar("SELECT last_value FROM event_store_sequence_seq")
            .fetch_one(&primary)
            .await
            .unwrap();
    sqlx::raw_sql(
        r#"
        CREATE OR REPLACE FUNCTION block_cross_campaign_probe()
        RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN
            IF NEW.event_type = 'ConcurrentAFirst' THEN
                PERFORM pg_advisory_xact_lock(90316001);
            ELSIF NEW.event_type = 'ConcurrentBFirst' THEN
                PERFORM pg_advisory_xact_lock(90316002);
            END IF;
            RETURN NEW;
        END;
        $$;
        DROP TRIGGER IF EXISTS block_cross_campaign_probe ON event_store;
        CREATE TRIGGER block_cross_campaign_probe
        BEFORE INSERT ON event_store
        FOR EACH ROW EXECUTE FUNCTION block_cross_campaign_probe();
        "#,
    )
    .execute(&primary)
    .await
    .unwrap();

    let mut concurrent_a = draft(
        "concurrent_a",
        0,
        &["ConcurrentAFirst", "ConcurrentASecond"],
    );
    bind_campaign(&mut concurrent_a, "campaign_concurrent_a");
    let mut concurrent_b = draft(
        "concurrent_b",
        0,
        &["ConcurrentBFirst", "ConcurrentBSecond"],
    );
    bind_campaign(&mut concurrent_b, "campaign_concurrent_b");
    let store_a = store.clone();
    let store_b = store.clone();
    let concurrent_a_task = tokio::spawn(async move { store_a.commit(&concurrent_a).await });
    let concurrent_b_task = tokio::spawn(async move { store_b.commit(&concurrent_b).await });

    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            let current: i64 =
                sqlx::query_scalar("SELECT last_value FROM event_store_sequence_seq")
                    .fetch_one(&primary)
                    .await
                    .unwrap();
            if current >= sequence_before + 2 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("both concurrent commits reached their deterministic barrier");
    let unlocked: (bool, bool) =
        sqlx::query_as("SELECT pg_advisory_unlock(90316001), pg_advisory_unlock(90316002)")
            .fetch_one(&mut barrier)
            .await
            .unwrap();
    assert_eq!(unlocked, (true, true));
    let concurrent_a_result = concurrent_a_task.await.unwrap();
    let concurrent_b_result = concurrent_b_task.await.unwrap();

    sqlx::raw_sql(
        r#"
        DROP TRIGGER IF EXISTS block_cross_campaign_probe ON event_store;
        DROP FUNCTION IF EXISTS block_cross_campaign_probe();
        "#,
    )
    .execute(&primary)
    .await
    .unwrap();
    concurrent_a_result.unwrap();
    concurrent_b_result.unwrap();

    let concurrent_a_sequences: Vec<i64> = sqlx::query_scalar(
        "SELECT event_sequence FROM event_outbox WHERE commit_id = 'concurrent_a' ORDER BY event_sequence",
    )
    .fetch_all(&primary)
    .await
    .unwrap();
    let concurrent_b_sequences: Vec<i64> = sqlx::query_scalar(
        "SELECT event_sequence FROM event_outbox WHERE commit_id = 'concurrent_b' ORDER BY event_sequence",
    )
    .fetch_all(&primary)
    .await
    .unwrap();
    assert_eq!(concurrent_a_sequences.len(), 2);
    assert_eq!(concurrent_b_sequences.len(), 2);
    assert!(
        concurrent_a_sequences[0] < concurrent_b_sequences[1]
            && concurrent_b_sequences[0] < concurrent_a_sequences[1],
        "probe must produce overlapping global sequence ranges"
    );

    // A stale write deliberately leaves an unresolved PREPARED witness. A
    // process carrying the wrong HMAC key must fail before appending an
    // ABORTED recovery record; otherwise one bad deployment permanently
    // poisons the append-only external witness.
    let mut wrong_key_probe = draft("wrong_key_recovery_probe", 1, &["MustNotCommit"]);
    bind_campaign(&mut wrong_key_probe, "campaign_wrong_key_recovery_probe");
    assert!(matches!(
        store.commit(&wrong_key_probe).await,
        Err(CanonicalStoreError::VersionConflict {
            expected: 1,
            actual: 0
        })
    ));
    let witness_rows_before_wrong_key =
        scalar(&witness, "SELECT count(*) FROM external_audit_witness").await;
    let wrong_key_store = PostgresCanonicalStore::connect(
        &primary_url,
        &witness_url,
        "wrong-canonical-integrity-key",
        &[0xee; 32],
        "p05-canonical-payload-key",
        PAYLOAD_KEY,
    )
    .await
    .unwrap();
    assert_eq!(
        wrong_key_store.prepare_for_service().await,
        Err(CanonicalStoreError::IntegrityViolation(
            "external_witness_hmac_mismatch"
        ))
    );
    assert_eq!(
        scalar(&witness, "SELECT count(*) FROM external_audit_witness").await,
        witness_rows_before_wrong_key,
        "wrong-key recovery must not mutate the append-only witness"
    );
    assert_eq!(
        store.recover().await.unwrap(),
        RecoveryReport {
            finalized: 0,
            aborted: 1,
        }
    );

    store.verify_integrity().await.unwrap();
    let audit_integrity_versions: Vec<i32> = sqlx::query_scalar(
        "SELECT DISTINCT integrity_version FROM canonical_audit_log ORDER BY integrity_version",
    )
    .fetch_all(&primary)
    .await
    .unwrap();
    assert_eq!(audit_integrity_versions, vec![3]);

    // Simulate a privileged restore that bypasses ordinary triggers. Version 3
    // binds occurred_at into the HMAC, so timestamp-only tampering is detected
    // even when the database append-only guard is deliberately bypassed.
    let mut audit_tamper = primary.begin().await.unwrap();
    sqlx::query("SET LOCAL session_replication_role = replica")
        .execute(&mut *audit_tamper)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE canonical_audit_log SET occurred_at = occurred_at + interval '1 microsecond' WHERE sequence = (SELECT min(sequence) FROM canonical_audit_log)",
    )
    .execute(&mut *audit_tamper)
    .await
    .unwrap();
    audit_tamper.commit().await.unwrap();
    assert_eq!(
        store.verify_integrity().await,
        Err(CanonicalStoreError::IntegrityViolation(
            "canonical_audit_hmac_mismatch"
        ))
    );
}
