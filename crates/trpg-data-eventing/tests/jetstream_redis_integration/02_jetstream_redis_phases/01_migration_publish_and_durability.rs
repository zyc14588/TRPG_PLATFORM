{
    let database_url = env::var("P02_EVENTING_DATABASE_URL")
        .expect("P02_EVENTING_DATABASE_URL is required for the real PostgreSQL gate");
    let witness_url = env::var("P02_EVENTING_WITNESS_DATABASE_URL")
        .expect("P02_EVENTING_WITNESS_DATABASE_URL is required for the real PostgreSQL gate");
    let nats_url =
        env::var("P02_NATS_URL").expect("P02_NATS_URL is required for the real JetStream gate");
    let redis_url =
        env::var("P02_REDIS_URL").expect("P02_REDIS_URL is required for the real Redis gate");
    let suffix = std::process::id();

    // Seed a genuine pending row under the frozen schema. The HEAD migration,
    // rather than test SQL, is solely responsible for assigning its explicit
    // historical classification.
    let pool = reset_to_frozen_event_store(&database_url, &witness_url).await;
    let legacy_sequence: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO event_store (
            event_type, command_id, idempotency_key, expected_version,
            authority_mode, authority_contract_version, visibility_label,
            fact_provenance_kind, fact_provenance_reference, fact_recorded_by,
            correlation_id, causation_id, payload_json
        ) VALUES (
            'ClueDiscovered', $1, $2, 0, 'human_kp', 1, 'keeper_only',
            'imported_source', 'frozen_schema_upgrade_fixture',
            'migration_upgrade', $3, $4, '{"clue":"harbor ledger"}'
        ) RETURNING sequence
        "#,
    )
    .bind(format!("upgrade_command_{suffix}"))
    .bind(format!("upgrade_event_{suffix}"))
    .bind(format!("upgrade_correlation_{suffix}"))
    .bind(format!("upgrade_causation_{suffix}"))
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO event_outbox (
            event_sequence, nats_subject, idempotency_key, visibility_label,
            correlation_id, causation_id, payload_json
        ) VALUES (
            $1, 'trpg.events.appended', $2, 'keeper_only', $3, $4,
            '{"clue":"harbor ledger"}'
        )
        "#,
    )
    .bind(legacy_sequence)
    .bind(format!("upgrade_outbox_{suffix}"))
    .bind(format!("upgrade_correlation_{suffix}"))
    .bind(format!("upgrade_causation_{suffix}"))
    .execute(&pool)
    .await
    .unwrap();

    // A second frozen-schema row carries CR/LF in values that become NATS
    // headers. Old deployments allowed these nonblank strings. HEAD must keep
    // the publisher alive, fail only this delivery, and continue the batch.
    let poisoned_header_sequence: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO event_store (
            event_type, command_id, idempotency_key, expected_version,
            authority_mode, authority_contract_version, visibility_label,
            fact_provenance_kind, fact_provenance_reference, fact_recorded_by,
            correlation_id, causation_id, payload_json
        ) VALUES (
            'ClueDiscovered', $1, $2, 0, 'human_kp', 1, 'keeper_only',
            'imported_source', 'frozen_header_upgrade_fixture',
            'migration_upgrade', $3, $4, '{"clue":"poisoned header"}'
        ) RETURNING sequence
        "#,
    )
    .bind(format!("poisoned_header_command_{suffix}"))
    .bind(format!("poisoned_header_event_{suffix}"))
    .bind(format!("poisoned\r\ncorrelation_{suffix}"))
    .bind(format!("poisoned_header_causation_{suffix}"))
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO event_outbox (
            event_sequence, nats_subject, idempotency_key, visibility_label,
            correlation_id, causation_id, payload_json
        ) VALUES (
            $1, 'trpg.events.appended', $2, 'keeper_only', $3, $4,
            '{"clue":"poisoned header"}'
        )
        "#,
    )
    .bind(poisoned_header_sequence)
    .bind(format!("poisoned\r\nmessage_id_{suffix}"))
    .bind(format!("poisoned\r\ncorrelation_{suffix}"))
    .bind(format!("poisoned_header_causation_{suffix}"))
    .execute(&pool)
    .await
    .unwrap();

    let store = PostgresCanonicalStore::connect(
        &database_url,
        &witness_url,
        "p02-jetstream-key",
        KEY,
        "p05-jetstream-payload-key",
        PAYLOAD_KEY,
    )
    .await
    .unwrap();
    store.prepare_for_service().await.unwrap();
    store.commit(&draft(suffix)).await.unwrap();

    // JetStream de-duplication is global to its NATS stream, while command
    // idempotency is scoped to campaign/resource stream. Both rows must be
    // published even though they intentionally reuse the same client key.
    let mut scoped_a = draft(suffix.saturating_add(1));
    scoped_a.commit_id = format!("jetstream_scoped_a_{suffix}");
    scoped_a.campaign_id = format!("jetstream_multistream_{suffix}");
    scoped_a.stream_id = format!("jetstream_scene_a_{suffix}");
    scoped_a.idempotency_key = format!("jetstream_shared_key_{suffix}");
    scoped_a.command_id = format!("jetstream_scoped_command_a_{suffix}");
    scoped_a.authority_contract_id = format!("jetstream_authority_multistream_{suffix}");
    scoped_a.audit.resource_type = "scene".to_owned();
    scoped_a.audit.resource_id = scoped_a.stream_id.clone();
    let mut scoped_b = draft(suffix.saturating_add(2));
    scoped_b.commit_id = format!("jetstream_scoped_b_{suffix}");
    scoped_b.campaign_id = scoped_a.campaign_id.clone();
    scoped_b.stream_id = format!("jetstream_scene_b_{suffix}");
    scoped_b.idempotency_key = scoped_a.idempotency_key.clone();
    scoped_b.command_id = format!("jetstream_scoped_command_b_{suffix}");
    scoped_b.authority_contract_id = scoped_a.authority_contract_id.clone();
    scoped_b.audit.resource_type = "scene".to_owned();
    scoped_b.audit.resource_id = scoped_b.stream_id.clone();
    store.commit(&scoped_a).await.unwrap();
    store.commit(&scoped_b).await.unwrap();

    let metrics = Arc::new(EventingMetrics::default());
    let publisher = JetStreamOutboxPublisher::connect(
        store.clone(),
        &nats_url,
        "p02-jetstream-publisher",
        None,
    )
    .await
    .unwrap()
    .with_metrics(Arc::clone(&metrics));

    // An existing stream is not accepted merely because it has a matching
    // subject. Every configured durability/de-duplication safety field must
    // match, otherwise startup fails closed without silently rewriting it.
    let nats_client = async_nats::connect(&nats_url).await.unwrap();
    let jetstream = async_nats::jetstream::new(nats_client.clone());

    // Prove the binary was compiled with the NATS 2.10 configuration surface:
    // a server-side subject transform must survive the client round trip. The
    // unit-level comparator then verifies that this single-field drift is
    // rejected against the canonical stream contract.
    let _ = jetstream.delete_stream("P04_SUBJECT_TRANSFORM_PROBE").await;
    let mut transform_probe = jetstream
        .create_stream(StreamConfig {
            name: "P04_SUBJECT_TRANSFORM_PROBE".to_owned(),
            subjects: vec!["p04.probe.>".to_owned()],
            subject_transform: Some(SubjectTransform {
                source: "p04.probe.>".to_owned(),
                destination: "p04.transformed.>".to_owned(),
            }),
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(
        transform_probe
            .info()
            .await
            .unwrap()
            .config
            .subject_transform,
        Some(SubjectTransform {
            source: "p04.probe.>".to_owned(),
            destination: "p04.transformed.>".to_owned(),
        })
    );
    jetstream
        .delete_stream("P04_SUBJECT_TRANSFORM_PROBE")
        .await
        .unwrap();

    let _ = jetstream.delete_stream("TRPG_CANONICAL_EVENTS").await;
    jetstream
        .create_stream(StreamConfig {
            name: "TRPG_CANONICAL_EVENTS".to_owned(),
            subjects: vec!["trpg.events.>".to_owned()],
            storage: StorageType::Memory,
            max_bytes: 1024,
            max_age: Duration::from_secs(60),
            duplicate_window: Duration::from_secs(1),
            deny_delete: false,
            deny_purge: false,
            ..Default::default()
        })
        .await
        .unwrap();
    assert_eq!(
        publisher.ensure_stream().await,
        Err(JetStreamOutboxError::Configuration(
            "jetstream_stream_contract_mismatch"
        ))
    );
    jetstream
        .delete_stream("TRPG_CANONICAL_EVENTS")
        .await
        .unwrap();

    let mut event_messages = nats_client
        .subscribe("trpg.events.appended.>")
        .await
        .unwrap();
    nats_client.flush().await.unwrap();
    publisher.ensure_stream().await.unwrap();
    let result = publisher.publish_batch().await.unwrap();
    assert_eq!(result.claimed, 3);
    assert_eq!(result.published, 3);
    assert_eq!(result.failed, 0);
    assert_eq!(result.dead_lettered, 0);
    assert_eq!(result.dead_letter_total, 2);
    assert!(result.requires_operator_attention());
    assert!(publisher.stream_message_count().await.unwrap() >= 3);
    assert_eq!(
        metrics.counter_value(EVENTING_COMMAND_TOTAL_METRIC, "outbox_publish", "published"),
        3
    );
    assert_eq!(
        metrics.counter_value(EVENTING_COMMAND_TOTAL_METRIC, "outbox_publish", "failed"),
        0
    );
    let formal_metric = metrics
        .observations()
        .into_iter()
        .find(|observation| observation.correlation_id == format!("jetstream_correlation_{suffix}"))
        .expect("formal outbox metric must retain its correlation context");
    assert_eq!(
        formal_metric.causation_id,
        format!("jetstream_causation_{suffix}")
    );
    assert_eq!(formal_metric.visibility_label, "keeper_only");
    assert_eq!(formal_metric.provenance_kind, "rules_engine_decision");

    // Validate the bytes that actually crossed NATS, rather than a helper
    // serialization detached from the publisher. All authoritative fields
    // live in the versioned shared-kernel envelope.
    let mut envelopes = Vec::with_capacity(result.published);
    for _ in 0..result.published {
        let message = tokio::time::timeout(Duration::from_secs(5), event_messages.next())
            .await
            .expect("published NATS event timed out")
            .expect("NATS event subscription ended");
        envelopes.push(
            serde_json::from_slice::<EventEnvelopeWire<serde_json::Value>>(&message.payload)
                .expect("publisher must emit the canonical event envelope"),
        );
    }
    for envelope in &envelopes {
        assert!(
            envelope.schema_version == EVENT_ENVELOPE_WIRE_SCHEMA_VERSION
                && envelope.event_schema_version > 0
                && envelope.sequence > 0
                && envelope.stream_version > 0
                && !envelope.authenticated_actor_id.is_empty()
                && !envelope.authenticated_actor_role.is_empty()
                && !envelope.authority_contract_id.is_empty()
                && !envelope.authority_owner.is_empty()
                && !envelope.command_id.is_empty()
                && !envelope.resource_type.is_empty()
                && !envelope.resource_id.is_empty()
                && !envelope.trace_id.is_empty()
                && envelope.occurred_at_unix_ms > 0,
            "incomplete production envelope: {envelope:?}"
        );
    }
    assert!(envelopes
        .iter()
        .all(|envelope| envelope.sequence != u64::try_from(legacy_sequence).unwrap()));
    let formal = envelopes
        .iter()
        .find(|envelope| envelope.campaign_id == format!("jetstream_campaign_{suffix}"))
        .expect("formal event was not published");
    assert_eq!(formal.authenticated_actor_id, "workflow_jetstream");
    assert_eq!(
        formal.event_schema_version,
        u32::try_from(CURRENT_EVENT_SCHEMA_VERSION).unwrap()
    );
    assert_eq!(formal.authenticated_actor_role, "workflow");
    assert!(matches!(
        formal.authenticated_actor_origin,
        EventActorOriginWire::Workload { ref role }
            if role == "workflow_engine"
    ));
    assert_eq!(formal.resource_campaign_id, formal.campaign_id);
    assert_eq!(formal.resource_type, "campaign");
    assert_eq!(formal.resource_id, formal.campaign_id);
    assert_eq!(formal.visibility_subject, None);
    assert_eq!(formal.request_hash_source, "formal_commit");
    assert_eq!(formal.integrity_status, "verified_hmac");
    assert!(formal.integrity_hash.is_some());
    assert!(formal.payload.get("protected_payload").is_some());
    let formal_wire = serde_json::to_string(formal).unwrap();
    assert!(!formal_wire.contains("harbor ledger"));
    assert_eq!(publisher.pending_count().await.unwrap(), 0);
    let legacy_delivery: (bool, bool, Option<String>, String) = sqlx::query_as(
        "SELECT published_at IS NULL, dead_lettered_at IS NOT NULL, last_error, integrity_status FROM event_outbox WHERE event_sequence = $1",
    )
    .bind(legacy_sequence)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        legacy_delivery,
        (
            true,
            true,
            Some("UNVERIFIED_HISTORY_QUARANTINED".to_owned()),
            "historical_unsigned".to_owned()
        )
    );
    let poisoned_header_delivery: (bool, i32, Option<String>, bool) = sqlx::query_as(
        "SELECT published_at IS NULL, retry_count, last_error, claim_owner IS NULL FROM event_outbox WHERE event_sequence = $1",
    )
    .bind(poisoned_header_sequence)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        poisoned_header_delivery,
        (
            true,
            0,
            Some("UNVERIFIED_HISTORY_QUARANTINED".to_owned()),
            true
        )
    );
    let persistent_alert = publisher.publish_batch().await.unwrap();
    assert_eq!(persistent_alert.dead_letter_total, 2);
    include!("02_retry_alerts_and_redis_model.rs");
}
