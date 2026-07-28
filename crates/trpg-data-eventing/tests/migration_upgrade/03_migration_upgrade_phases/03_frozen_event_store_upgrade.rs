{
    sqlx::query(
        r#"
        INSERT INTO event_outbox (
            event_sequence, nats_subject, idempotency_key, visibility_label,
            correlation_id, causation_id, payload_json
        ) VALUES ($1, 'trpg.events.appended', 'legacy_outbox:0000',
                  'party_visible', 'legacy_correlation', 'legacy_causation',
                  '{"legacy":true}')
        "#,
    )
    .bind(legacy_sequence)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO projection_checkpoint (projection_name, last_event_sequence, projection_hash) VALUES ('legacy_projection', $1, 'legacy_hash')",
    )
    .bind(legacy_sequence)
    .execute(&pool)
    .await
    .unwrap();

    current
        .run(&pool)
        .await
        .expect("b-24 upgrades without VersionMismatch");
    assert_schema(&pool).await;

    let event: EventStoreRecord = sqlx::query_as("SELECT * FROM event_store WHERE sequence = $1")
        .bind(legacy_sequence)
        .fetch_one(&pool)
        .await
        .expect("lossless SQLx event mapping");
    assert_eq!(event.payload_json, json!({"legacy": true}));
    assert_eq!(event.campaign_id, "historical_unscoped");
    assert_eq!(event.stream_id, "historical_unscoped");
    assert_eq!(event.event_schema_version, 1);
    assert_eq!(event.request_hash_source, "historical_unavailable");
    assert_eq!(event.integrity_status, "historical_unsigned");
    assert_eq!(event.event_integrity_hash, None);
    assert_eq!(event.payload_integrity_source, r#"{"legacy":true}"#);
    let serde_round_trip: EventStoreRecord =
        serde_json::from_value(serde_json::to_value(&event).unwrap()).unwrap();
    assert_eq!(serde_round_trip, event);

    let outbox: EventOutboxRecord =
        sqlx::query_as("SELECT * FROM event_outbox WHERE event_sequence = $1")
            .bind(legacy_sequence)
            .fetch_one(&pool)
            .await
            .expect("lossless SQLx outbox mapping");
    assert_eq!(outbox.event_id, legacy_sequence);
    assert_eq!(outbox.payload_json, json!({"legacy": true}));
    assert_eq!(outbox.request_hash_source, "historical_unavailable");
    assert_eq!(outbox.integrity_status, "historical_unsigned");
    assert_eq!(outbox.delivery_status, "dead_lettered");
    assert_eq!(
        outbox.last_error.as_deref(),
        Some("UNVERIFIED_HISTORY_QUARANTINED")
    );
    assert!(outbox.dead_lettered_at.is_some());
    assert!(outbox.locked_until.is_none());
    assert!(outbox.claim_token.is_none());
    let outbox_serde_round_trip: EventOutboxRecord =
        serde_json::from_value(serde_json::to_value(&outbox).unwrap()).unwrap();
    assert_eq!(outbox_serde_round_trip, outbox);
    assert!(
        sqlx::query("DELETE FROM event_outbox WHERE event_sequence = $1")
            .bind(legacy_sequence)
            .execute(&pool)
            .await
            .is_err()
    );
    assert!(sqlx::query("TRUNCATE event_outbox")
        .execute(&pool)
        .await
        .is_err());
    let delivery_state_error = sqlx::query(
        "UPDATE event_outbox SET delivery_status = 'claimed', claimed_at = now(), claim_owner = 'p04_probe', claim_token = 'claim-sha256:migration-upgrade-probe', locked_until = now() + interval '60 seconds' WHERE event_sequence = $1",
    )
    .bind(legacy_sequence)
    .execute(&pool)
    .await
    .expect_err("quarantined historical delivery cannot be reclaimed");
    assert!(delivery_state_error
        .to_string()
        .contains("terminal outbox delivery evidence is immutable"));
    for identity_mutation in [
        "UPDATE event_outbox SET outbox_id = outbox_id + 1000 WHERE event_sequence = $1",
        "UPDATE event_outbox SET idempotency_key = idempotency_key || ':rebound' WHERE event_sequence = $1",
        "UPDATE event_outbox SET commit_id = 'rebound_commit' WHERE event_sequence = $1",
    ] {
        let error = sqlx::query(identity_mutation)
            .bind(legacy_sequence)
            .execute(&pool)
            .await
            .expect_err("canonical outbox identity must be immutable");
        assert!(error
            .to_string()
            .contains("canonical outbox identity is immutable"));
    }

    let checkpoint: ProjectionCheckpointRecord = sqlx::query_as(
        "SELECT * FROM projection_checkpoint WHERE projection_name = 'legacy_projection'",
    )
    .fetch_one(&pool)
    .await
    .expect("lossless SQLx checkpoint mapping");
    assert_eq!(checkpoint.version, 0);
    assert_eq!(checkpoint.last_event_sequence, 0);
    assert_eq!(checkpoint.stream_id, "historical_unscoped");
    assert_eq!(
        checkpoint.projection_hash,
        "sha256:0000000000000000000000000000000000000000000000000000000000000000"
    );
    let checkpoint_serde_round_trip: ProjectionCheckpointRecord =
        serde_json::from_value(serde_json::to_value(&checkpoint).unwrap()).unwrap();
    assert_eq!(checkpoint_serde_round_trip, checkpoint);

    let upcasted = EventPayloadUpcaster::canonical()
        .upcast(
            &event.event_type,
            event.event_schema_version,
            event.payload_json,
        )
        .expect("known historical version upcasts");
    assert_eq!(upcasted.event_schema_version, CURRENT_EVENT_SCHEMA_VERSION);
    assert_eq!(upcasted.payload, json!({"legacy": true}));
    assert!(EventPayloadUpcaster::canonical()
        .upcast("CampaignStarted", 99, Value::Null)
        .is_err());

    let payload_cipher = PayloadCipher::new("migration_test_payload", &[0x91; 32]).unwrap();
    let replayed =
        load_canonical_replay_page(&pool, &payload_cipher, "historical_unscoped", 0, 100)
            .await
            .expect("production replay path omits explicitly unsigned historical data");
    assert!(replayed.is_empty());
    let legacy_projection = PostgresProjectionWorker::new(pool.clone(), "legacy_projection", 100)
        .expect("construct upgraded projection worker");
    let rebuilt = legacy_projection
        .rebuild_to_tip("historical_unscoped", "historical_unscoped")
        .await
        .expect("genesis-reset legacy checkpoint quarantines unverified history");
    assert_eq!(rebuilt.version, 0);
    assert_eq!(rebuilt.last_event_sequence, 0);
    let projected_legacy_rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM canonical_event_projection WHERE projection_name = 'legacy_projection'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(projected_legacy_rows, 0);

    // Invalid JSON, enum, negative version, and blank IDs are rejected by the
    // database. Scoped idempotency is covered by the exact constraint
    // signature here and by the real canonical-commit integration test.
    let mixed_integrity_metadata = sqlx::query(
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
            integrity_status, payload_integrity_source
        ) VALUES (
            'MixedIntegrityProbe', 'mixed_integrity_command',
            'mixed_integrity_event', 0, 'human_kp', 1, 'party_visible',
            'imported_source', 'migration_constraint_probe', 'migration_upgrade',
            'mixed_integrity_correlation', 'mixed_integrity_causation', '{}'::jsonb,
            'historical_unscoped', $1, 'historical_unknown', 'historical_unknown',
            '{"kind":"workload","role":"historical_unknown"}'::jsonb,
            'historical_unknown',
            'historical_unknown', 'historical_unknown', 'historical_unknown',
            'not_applicable', 'historical_unknown',
            'hmac-sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd',
            'historical_unscoped', 1, 'canonical_commit', $2,
            'historical_unavailable', 'verified_hmac', '{}'
        )
        "#,
    )
    .bind(legacy_sequence + 1)
    .bind(ZERO_REQUEST_HASH)
    .execute(&pool)
    .await;
    assert!(mixed_integrity_metadata.is_err());
    assert!(insert_event(
        &pool,
        EventInsert {
            payload_json: "not-json",
            ..EventInsert::valid("campaign_invalid_json", "stream_invalid_json")
        },
    )
    .await
    .is_err());
    assert!(insert_event(
        &pool,
        EventInsert {
            authority_mode: "forged_kp",
            ..EventInsert::valid("campaign_invalid_enum", "stream_invalid_enum")
        },
    )
    .await
    .is_err());
    assert!(insert_event(
        &pool,
        EventInsert {
            expected_version: -1,
            ..EventInsert::valid("campaign_negative", "stream_negative")
        },
    )
    .await
    .is_err());
    assert!(insert_event(
        &pool,
        EventInsert {
            visibility_label: "all_players_and_keeper_secrets",
            ..EventInsert::valid("campaign_invalid_visibility", "stream_invalid_visibility")
        },
    )
    .await
    .is_err());
    assert!(insert_event(
        &pool,
        EventInsert {
            provenance_kind: "invented_by_agent",
            ..EventInsert::valid("campaign_invalid_provenance", "stream_invalid_provenance")
        },
    )
    .await
    .is_err());
    assert!(insert_event(
        &pool,
        EventInsert {
            payload_integrity_source: r#"{"different":true}"#,
            ..EventInsert::valid("campaign_payload_mismatch", "stream_payload_mismatch")
        },
    )
    .await
    .is_err());
    assert!(
        insert_event(&pool, EventInsert::valid("campaign_blank", " "))
            .await
            .is_err()
    );
    let orphan_error = insert_event(
        &pool,
        EventInsert::valid("campaign_orphan_current", "stream_orphan_current"),
    )
    .await
    .expect_err("a current event cannot commit without its outbox/formal marker");
    assert!(orphan_error
        .to_string()
        .contains("formal event lacks one complete outbox/commit binding"));

    // Historical classification is migration output, not an application
    // write mode. A post-HEAD Event + Outbox pair used to bypass the formal
    // commit and HMAC invariants; reject both insertion points while retaining
    // the genuine b-24 rows that were classified during the migration above.
    let mut historical_event_transaction = pool.begin().await.unwrap();
    let historical_event_error = try_insert_historical_probe_event(
        &mut historical_event_transaction,
        "post_head_historical_event",
        legacy_sequence + 1,
    )
    .await
    .expect_err("post-HEAD historical events must be rejected at insertion");
    assert!(historical_event_error
        .to_string()
        .contains("historical classification is migration-only"));
    historical_event_transaction.rollback().await.unwrap();

    let mut unverified_hmac_transaction = pool.begin().await.unwrap();
    let unverified_hmac_error = try_insert_hmac_probe_event(
        &mut unverified_hmac_transaction,
        "post_head_unverified_hmac_event",
        "post_head_unverified_hmac_campaign",
        "post_head_unverified_hmac_stream",
        "historical_unverified_hmac",
    )
    .await
    .expect_err("post-HEAD unverified historical HMAC events must be rejected at insertion");
    assert!(unverified_hmac_error
        .to_string()
        .contains("historical classification is migration-only"));
    unverified_hmac_transaction.rollback().await.unwrap();

    let historical_outbox_error = sqlx::query(
        r#"
        INSERT INTO event_outbox (
            event_id, event_sequence, nats_subject, idempotency_key,
            visibility_label, correlation_id, causation_id, payload_json,
            retry_count, campaign_id, stream_id, event_schema_version,
            idempotency_operation, request_hash, request_hash_source,
            integrity_status
        ) VALUES (
            $1, $1, 'trpg.events.appended', 'post_head_historical_outbox',
            'party_visible', 'legacy_correlation', 'legacy_causation',
            '{"legacy":true}'::jsonb, 0, 'historical_unscoped',
            'historical_unscoped', 1, 'canonical_commit', $2,
            'historical_unavailable', 'historical_unsigned'
        )
        "#,
    )
    .bind(legacy_sequence)
    .bind(ZERO_REQUEST_HASH)
    .execute(&pool)
    .await
    .expect_err("post-HEAD historical outboxes must be rejected at insertion");
    assert!(historical_outbox_error
        .to_string()
        .contains("historical classification is migration-only"));

    let bad_reference = sqlx::query(
        r#"
        INSERT INTO event_outbox (
            event_id, event_sequence, nats_subject, idempotency_key,
            visibility_label, correlation_id, causation_id, payload_json,
            retry_count, campaign_id, stream_id, event_schema_version,
            idempotency_operation, request_hash, request_hash_source,
            integrity_status
        ) VALUES (
            999999999, 999999999, 'trpg.events.appended', 'bad_reference',
            'party_visible', 'correlation', 'causation', '{}'::jsonb, 0,
            'missing_campaign', 'missing_stream', 1,
            'canonical_commit', $1, 'formal_commit', 'verified_hmac'
        )
        "#,
    )
    .bind(REQUEST_HASH_A)
    .execute(&pool)
    .await;
    assert!(bad_reference.is_err());

    let mut negative_retry_transaction = pool.begin().await.unwrap();
    include!("04_outbox_retry_and_visibility_constraints.rs");
}
