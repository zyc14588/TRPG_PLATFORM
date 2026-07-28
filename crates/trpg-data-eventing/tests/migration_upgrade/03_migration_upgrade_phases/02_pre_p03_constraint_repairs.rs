{
    migrator_before_p03(current)
        .run(&pool)
        .await
        .expect("apply the canonical schema before audit-trigger drift probe");
    pool.execute("DROP TRIGGER canonical_audit_log_append_only ON canonical_audit_log")
        .await
        .unwrap();
    let audit_trigger_error = current
        .run(&pool)
        .await
        .expect_err("missing audit mutation guard must fail before P03");
    assert!(audit_trigger_error
        .to_string()
        .contains("mutation trigger drift"));
    assert_hardening_not_recorded(&pool).await;
    pool.close().await;
    pool = test_pool().await;

    reset_database(&pool).await;
    migrator_before_p03(current)
        .run(&pool)
        .await
        .expect("apply the canonical schema before audit-chain drift probe");
    sqlx::raw_sql(
        r#"
        CREATE OR REPLACE FUNCTION enforce_canonical_audit_chain()
        RETURNS trigger LANGUAGE plpgsql AS $body$
        BEGIN
            RETURN NEW;
        END;
        $body$;
        "#,
    )
    .execute(&pool)
    .await
    .unwrap();
    let audit_chain_error = current
        .run(&pool)
        .await
        .expect_err("audit-chain function drift must fail before P03");
    assert!(audit_chain_error
        .to_string()
        .contains("chain function drift"));
    assert_hardening_not_recorded(&pool).await;
    pool.close().await;
    pool = test_pool().await;

    // A formal commit's global sequence bounds may overlap another campaign's
    // bounds. Request hashes therefore have to follow the exact
    // event_outbox.commit_id binding, never a BETWEEN range over event_store.
    reset_database(&pool).await;
    migrator_before_p03(current)
        .run(&pool)
        .await
        .expect("apply the canonical schema immediately before P03");
    let a1 = insert_pre_p03_event(&pool, "campaign_interleave_a", "event_a_1", 1).await;
    let b1 = insert_pre_p03_event(&pool, "campaign_interleave_b", "event_b_1", 1).await;
    let b2 = insert_pre_p03_event(&pool, "campaign_interleave_b", "event_b_2", 2).await;
    let a2 = insert_pre_p03_event(&pool, "campaign_interleave_a", "event_a_2", 2).await;
    insert_pre_p03_outbox(&pool, a1, "outbox_a_1", "commit_a").await;
    insert_pre_p03_outbox(&pool, b1, "outbox_b_1", "commit_b").await;
    insert_pre_p03_outbox(&pool, b2, "outbox_b_2", "commit_b").await;
    insert_pre_p03_outbox(&pool, a2, "outbox_a_2", "commit_a").await;
    let audit_a = insert_pre_p03_audit(&pool, "commit_a", "campaign_interleave_a", 'a').await;
    let audit_b = insert_pre_p03_audit(&pool, "commit_b", "campaign_interleave_b", 'b').await;
    assert_eq!((audit_a, audit_b), (1, 2));
    insert_pre_p03_formal(
        &pool,
        "commit_a",
        "campaign_interleave_a",
        REQUEST_HASH_A,
        a1,
        a2,
        audit_a,
    )
    .await;
    insert_pre_p03_formal(
        &pool,
        "commit_b",
        "campaign_interleave_b",
        REQUEST_HASH_B,
        b1,
        b2,
        audit_b,
    )
    .await;
    current
        .run(&pool)
        .await
        .expect("interleaved pre-P03 data upgrades");
    let mismatched_request_hashes: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*)
          FROM event_store AS event
          JOIN event_outbox AS outbox ON outbox.event_sequence = event.sequence
          JOIN formal_commits AS formal ON formal.commit_id = outbox.commit_id
         WHERE event.request_hash <> formal.request_hash
            OR outbox.request_hash <> formal.request_hash
        "#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        mismatched_request_hashes, 0,
        "P03 must not bind an interleaved event to another campaign's request"
    );
    let upgraded_integrity_states: Vec<(String, String)> = sqlx::query_as(
        r#"
        SELECT DISTINCT event.integrity_status, outbox.integrity_status
          FROM event_store AS event
          JOIN event_outbox AS outbox ON outbox.event_sequence = event.sequence
         WHERE event.event_integrity_hash IS NOT NULL
         ORDER BY event.integrity_status, outbox.integrity_status
        "#,
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        upgraded_integrity_states,
        vec![(
            "historical_unverified_hmac".to_owned(),
            "historical_unverified_hmac".to_owned()
        )],
        "migration must not claim cryptographic verification it did not perform"
    );
    let audit_integrity_versions: Vec<i32> =
        sqlx::query_scalar("SELECT DISTINCT integrity_version FROM canonical_audit_log")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(audit_integrity_versions, vec![1]);
    let audit_occurred_at_before: String =
        sqlx::query_scalar("SELECT occurred_at::text FROM canonical_audit_log WHERE sequence = $1")
            .bind(audit_a)
            .fetch_one(&pool)
            .await
            .unwrap();
    let audit_update_error = sqlx::query(
        "UPDATE canonical_audit_log SET occurred_at = occurred_at + interval '1 second' WHERE sequence = $1",
    )
    .bind(audit_a)
    .execute(&pool)
    .await
    .expect_err("audit timestamp must remain append-only after upgrade");
    assert!(audit_update_error
        .to_string()
        .contains("canonical commit records are append-only"));
    let audit_occurred_at_after: String =
        sqlx::query_scalar("SELECT occurred_at::text FROM canonical_audit_log WHERE sequence = $1")
            .bind(audit_a)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(audit_occurred_at_before, audit_occurred_at_after);
    assert!(
        sqlx::query("DELETE FROM canonical_audit_log WHERE sequence = $1")
            .bind(audit_a)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(sqlx::query("TRUNCATE canonical_audit_log")
        .execute(&pool)
        .await
        .is_err());

    // A committed marker freezes its exact Event/Outbox set. Reusing the
    // existing commit_id for a later pair must fail at deferred validation,
    // even when campaign, stream, request hash, and per-row metadata match.
    let mut commit_reuse_transaction = pool.begin().await.unwrap();
    let reused_sequence: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO event_store (
            event_type, command_id, idempotency_key, expected_version,
            authority_mode, authority_contract_version, visibility_label,
            fact_provenance_kind, fact_provenance_reference, fact_recorded_by,
            correlation_id, causation_id, payload_json, campaign_id,
            stream_version, authenticated_actor_id, authenticated_actor_role,
            authenticated_actor_origin, resource_type, resource_id,
            authority_contract_id, authority_owner, visibility_subject, trace_id,
            event_integrity_hash, stream_id, event_schema_version,
            idempotency_operation, request_hash, request_hash_source,
            integrity_status, payload_integrity_source, payload_ciphertext,
            payload_key_reference, payload_nonce
        ) VALUES (
            'CommitReuseProbe', 'commit_reuse_command', 'commit_reuse_event', 2,
            'human_kp', 1, 'party_visible', 'rules_engine_decision',
            'commit_reuse_probe', 'migration_upgrade', 'commit_reuse_correlation',
            'commit_reuse_causation', $2::jsonb, 'campaign_interleave_a', 3,
            'keeper', 'workflow',
            '{"kind":"workload","role":"workflow_engine"}'::jsonb,
            'campaign', 'campaign_interleave_a', 'authority_fixture',
            'keeper', 'not_applicable', 'commit_reuse_trace',
            'hmac-sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd',
            'campaign_interleave_a', 1, 'canonical_commit', $1,
            'formal_commit', 'verified_hmac', $2,
            decode(repeat('00', 16), 'hex'), 'migration_fixture_key',
            decode(repeat('00', 12), 'hex')
        ) RETURNING sequence
        "#,
    )
    .bind(REQUEST_HASH_A)
    .bind(PROTECTED_PAYLOAD_FIXTURE)
    .fetch_one(&mut *commit_reuse_transaction)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO event_outbox (
            event_id, event_sequence, nats_subject, idempotency_key,
            visibility_label, correlation_id, causation_id, payload_json,
            commit_id, campaign_id, stream_id, event_schema_version,
            idempotency_operation, request_hash, request_hash_source,
            integrity_status, visibility_subject, payload_ciphertext,
            payload_key_reference, payload_nonce
        ) VALUES (
            $1, $1, 'trpg.events.appended', 'commit_reuse_outbox',
            'party_visible', 'commit_reuse_correlation',
            'commit_reuse_causation', $3::jsonb, 'commit_a',
            'campaign_interleave_a', 'campaign_interleave_a', 1,
            'canonical_commit', $2, 'formal_commit', 'verified_hmac',
            'not_applicable', decode(repeat('00', 16), 'hex'),
            'migration_fixture_key', decode(repeat('00', 12), 'hex')
        )
        "#,
    )
    .bind(reused_sequence)
    .bind(REQUEST_HASH_A)
    .bind(PROTECTED_PAYLOAD_FIXTURE)
    .execute(&mut *commit_reuse_transaction)
    .await
    .unwrap();
    let commit_reuse_error = sqlx::query("SET CONSTRAINTS ALL IMMEDIATE")
        .execute(&mut *commit_reuse_transaction)
        .await
        .expect_err("a committed marker cannot acquire a later Event/Outbox pair");
    assert!(commit_reuse_error
        .to_string()
        .contains("formal commit exact event/outbox set changed after commit"));
    commit_reuse_transaction.rollback().await.unwrap();

    // Before child-owned v2 materialization, fork lineage events were stored
    // on the parent campaign stream. One parent can legitimately have several
    // immutable children, so the v2 child uniqueness index must exclude those
    // rows while upgrading an already-populated database.
    reset_database(&pool).await;
    migrator_through(current, 20260727000600)
        .run(&pool)
        .await
        .expect("apply schema immediately before child-owned lineage index");
    sqlx::query("ALTER TABLE public.event_store DISABLE TRIGGER USER")
        .execute(&pool)
        .await
        .unwrap();
    for (stream_id, idempotency_key) in [
        ("legacy_parent_fork_child_a", "legacy_parent_fork_a"),
        ("legacy_parent_fork_child_b", "legacy_parent_fork_b"),
    ] {
        insert_event(
            &pool,
            EventInsert {
                idempotency_key,
                ..EventInsert::valid("legacy_fork_parent", stream_id)
            },
        )
        .await
        .expect("seed a verified legacy parent-owned fork event");
    }
    sqlx::query(
        "UPDATE public.event_store \
         SET event_type = 'CampaignForkRecorded' \
         WHERE campaign_id = 'legacy_fork_parent'",
    )
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("ALTER TABLE public.event_store ENABLE TRIGGER USER")
        .execute(&pool)
        .await
        .unwrap();

    current
        .run(&pool)
        .await
        .expect("multiple legacy parent-owned forks upgrade to child-owned v2");
    let legacy_parent_forks: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM public.event_store \
         WHERE campaign_id = 'legacy_fork_parent' \
           AND event_type = 'CampaignForkRecorded'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(legacy_parent_forks, 2);
    let child_lineage_index: String = sqlx::query_scalar(
        "SELECT indexdef FROM pg_indexes \
         WHERE schemaname = 'public' \
           AND indexname = 'event_store_one_fork_lineage_per_child_idx'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(child_lineage_index.contains("public.campaign_fork_materializations"));

    // A genuine b-24 SQLx ledger and data set upgrades without checksum edits.
    reset_database(&pool).await;
    b24.run(&pool)
        .await
        .expect("apply historical b-24 migration");

    let legacy_sequence: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO event_store (
            event_type, command_id, idempotency_key, expected_version,
            authority_mode, authority_contract_version, visibility_label,
            fact_provenance_kind, fact_provenance_reference, fact_recorded_by,
            correlation_id, causation_id, payload_json
        ) VALUES (
            'CampaignStarted', 'legacy_command', 'legacy_idem:0000', 0,
            'human_kp', 1, 'party_visible', 'rules_engine_decision',
            'legacy_reference', 'legacy_keeper', 'legacy_correlation',
            'legacy_causation', '{"legacy":true}'
        ) RETURNING sequence
        "#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    include!("03_frozen_event_store_upgrade.rs");
}
