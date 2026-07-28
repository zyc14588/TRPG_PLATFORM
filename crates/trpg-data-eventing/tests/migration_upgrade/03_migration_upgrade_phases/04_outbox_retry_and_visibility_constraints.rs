{
    let negative_retry_event = insert_formal_probe_event(
        &mut negative_retry_transaction,
        "negative_retry_event",
        "negative_retry_campaign",
        "negative_retry_stream",
    )
    .await;
    let negative_retry = sqlx::query(
        r#"
        INSERT INTO event_outbox (
            event_id, event_sequence, nats_subject, idempotency_key,
            visibility_label, correlation_id, causation_id, payload_json,
            retry_count, campaign_id, stream_id, event_schema_version,
            idempotency_operation, request_hash, request_hash_source,
            integrity_status, visibility_subject, payload_ciphertext,
            payload_key_reference, payload_nonce
        ) VALUES (
            $1, $1, 'trpg.events.appended', 'negative_retry',
            'party_visible', 'correlation_probe', 'causation_probe', $3::jsonb, -1,
            'negative_retry_campaign', 'negative_retry_stream', 1,
            'canonical_commit', $2, 'formal_commit', 'verified_hmac',
            'not_applicable', decode(repeat('00', 16), 'hex'),
            'migration_fixture_key', decode(repeat('00', 12), 'hex')
        )
        "#,
    )
    .bind(negative_retry_event)
    .bind(REQUEST_HASH_A)
    .bind(PROTECTED_PAYLOAD_FIXTURE)
    .execute(&mut *negative_retry_transaction)
    .await;
    assert!(negative_retry.is_err());
    negative_retry_transaction.rollback().await.unwrap();

    let mut invalid_subject_transaction = pool.begin().await.unwrap();
    let invalid_subject_event = insert_formal_probe_event(
        &mut invalid_subject_transaction,
        "invalid_subject_event",
        "invalid_subject_campaign",
        "invalid_subject_stream",
    )
    .await;
    let invalid_subject = sqlx::query(
        r#"
        INSERT INTO event_outbox (
            event_id, event_sequence, nats_subject, idempotency_key,
            visibility_label, correlation_id, causation_id, payload_json,
            retry_count, campaign_id, stream_id, event_schema_version,
            idempotency_operation, request_hash, request_hash_source,
            integrity_status, visibility_subject, payload_ciphertext,
            payload_key_reference, payload_nonce
        ) VALUES (
            $1, $1, 'trpg.events.forged', 'invalid_subject',
            'party_visible', 'correlation_probe', 'causation_probe', $3::jsonb, 0,
            'invalid_subject_campaign', 'invalid_subject_stream', 1,
            'canonical_commit', $2, 'formal_commit', 'verified_hmac',
            'not_applicable', decode(repeat('00', 16), 'hex'),
            'migration_fixture_key', decode(repeat('00', 12), 'hex')
        )
        "#,
    )
    .bind(invalid_subject_event)
    .bind(REQUEST_HASH_A)
    .bind(PROTECTED_PAYLOAD_FIXTURE)
    .execute(&mut *invalid_subject_transaction)
    .await;
    assert!(invalid_subject.is_err());
    invalid_subject_transaction.rollback().await.unwrap();

    let mut mismatched_outbox_transaction = pool.begin().await.unwrap();
    let mismatched_outbox_event = insert_formal_probe_event(
        &mut mismatched_outbox_transaction,
        "mismatched_outbox_event",
        "mismatched_outbox_campaign",
        "mismatched_outbox_stream",
    )
    .await;
    let mismatched_outbox = sqlx::query(
        r#"
        INSERT INTO event_outbox (
            event_id, event_sequence, nats_subject, idempotency_key,
            visibility_label, correlation_id, causation_id, payload_json,
            retry_count, campaign_id, stream_id, event_schema_version,
            idempotency_operation, request_hash, request_hash_source,
            integrity_status, visibility_subject, payload_ciphertext,
            payload_key_reference, payload_nonce
        ) VALUES (
            $1, $1, 'trpg.events.appended', 'mismatched_outbox',
            'keeper_only', 'correlation_probe', 'causation_probe', $3::jsonb, 0,
            'mismatched_outbox_campaign', 'mismatched_outbox_stream', 1,
            'canonical_commit', $2, 'formal_commit', 'verified_hmac',
            'not_applicable', decode(repeat('00', 16), 'hex'),
            'migration_fixture_key', decode(repeat('00', 12), 'hex')
        )
        "#,
    )
    .bind(mismatched_outbox_event)
    .bind(REQUEST_HASH_A)
    .bind(PROTECTED_PAYLOAD_FIXTURE)
    .execute(&mut *mismatched_outbox_transaction)
    .await;
    assert!(mismatched_outbox.is_err());
    mismatched_outbox_transaction.rollback().await.unwrap();

    let ledger_before_repeat: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    current
        .run(&pool)
        .await
        .expect("upgraded database repeat no-op");
    let ledger_after_repeat: i64 = sqlx::query_scalar("SELECT count(*) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(ledger_before_repeat, ledger_after_repeat);
}
