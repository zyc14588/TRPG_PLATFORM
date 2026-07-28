{
    let (primary_url, witness_url) = database_urls();
    assert_distinct_database_targets(&primary_url, &witness_url);
    reset_dedicated_database(&primary_url, "P02_CANONICAL_RESET_DATABASE").await;
    reset_dedicated_database(&witness_url, "P02_CANONICAL_WITNESS_RESET_DATABASE").await;

    let store = PostgresCanonicalStore::connect(
        &primary_url,
        &witness_url,
        "p02-canonical-test-key",
        KEY,
        "p05-canonical-payload-key",
        PAYLOAD_KEY,
    )
    .await
    .unwrap();
    let (first_startup, concurrent_startup) =
        tokio::join!(store.prepare_for_service(), store.prepare_for_service());
    first_startup.unwrap();
    concurrent_startup.unwrap();

    let primary = PgPool::connect(&primary_url).await.unwrap();
    let witness = PgPool::connect(&witness_url).await.unwrap();

    let mut success_draft = draft("success", 0, &["CampaignStarted", "InvestigatorJoined"]);
    success_draft.events[0].payload_json = "{ \"z\": 1, \"a\": [true, null] }".to_owned();
    let success = store.commit(&success_draft).await.unwrap();
    assert_eq!(success.first_stream_version, 1);
    assert_eq!(success.last_stream_version, 2);

    // The synchronous production port resolves committed custody before a
    // caller repeats any non-idempotent external work. A miss also preflights
    // the stream version so stale commands fail before their executor runs.
    let retained_runtime = Arc::new(Mutex::new(
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap(),
    ));
    let canonical_port =
        PostgresCanonicalCommitPort::new(Arc::clone(&retained_runtime), store.clone());
    let exact_receipt = canonical_port
        .load_receipt(&CanonicalCommitKey {
            commit_id: success_draft.commit_id.clone(),
            campaign_id: success_draft.campaign_id.clone(),
            stream_id: success_draft.stream_id.clone(),
            idempotency_key: success_draft.idempotency_key.clone(),
            expected_version: 0,
        })
        .unwrap()
        .expect("committed canonical receipt must be reusable");
    assert_eq!(exact_receipt.first_stream_version, 1);
    assert_eq!(exact_receipt.last_stream_version, 2);
    assert_eq!(exact_receipt.events.len(), 2);
    assert!(canonical_port
        .load_receipt(&CanonicalCommitKey {
            commit_id: "future_commit".to_owned(),
            campaign_id: success_draft.campaign_id.clone(),
            stream_id: success_draft.stream_id.clone(),
            idempotency_key: "future_idempotency".to_owned(),
            expected_version: 2,
        })
        .unwrap()
        .is_none());
    assert_eq!(
        canonical_port.load_receipt(&CanonicalCommitKey {
            commit_id: "stale_commit".to_owned(),
            campaign_id: success_draft.campaign_id.clone(),
            stream_id: success_draft.stream_id.clone(),
            idempotency_key: "stale_idempotency".to_owned(),
            expected_version: 1,
        }),
        Err(TrpgError::ExpectedVersionConflict {
            expected: 1,
            actual: 2,
        })
    );
    drop(canonical_port);
    std::thread::spawn(move || drop(retained_runtime))
        .join()
        .unwrap();

    assert_eq!(
        scalar(&primary, "SELECT count(*) FROM event_store").await,
        2
    );
    assert_eq!(
        scalar(&primary, "SELECT count(*) FROM event_outbox").await,
        2
    );
    assert_eq!(
        scalar(&primary, "SELECT count(*) FROM canonical_audit_log").await,
        1
    );
    let audit_context: (String, String) = sqlx::query_as(
        "SELECT correlation_id, causation_id FROM canonical_audit_log WHERE commit_id = 'success'",
    )
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(
        audit_context,
        (
            success_draft.correlation_id.clone(),
            success_draft.causation_id.clone()
        )
    );
    assert_eq!(
        scalar(&primary, "SELECT count(*) FROM formal_commits").await,
        1
    );
    assert_eq!(
        scalar(&witness, "SELECT count(*) FROM external_audit_witness").await,
        2
    );
    let formal_commit: FormalCommitRecord =
        sqlx::query_as("SELECT * FROM formal_commits WHERE commit_id = 'success'")
            .fetch_one(&primary)
            .await
            .expect("lossless SQLx formal-commit mapping");
    let formal_commit_round_trip: FormalCommitRecord =
        serde_json::from_value(serde_json::to_value(&formal_commit).unwrap()).unwrap();
    assert_eq!(formal_commit_round_trip, formal_commit);
    assert_eq!(formal_commit.status, "committed");
    assert_eq!(
        formal_commit.result_event_sequence,
        success.last_event_sequence
    );
    let signed_payload: String =
        sqlx::query_scalar("SELECT payload_integrity_source FROM event_store WHERE sequence = $1")
            .bind(success.first_event_sequence)
            .fetch_one(&primary)
            .await
            .unwrap();
    assert_ne!(signed_payload, success_draft.events[0].payload_json);
    let protected_envelope = serde_json::from_str::<serde_json::Value>(&signed_payload).unwrap();
    assert_eq!(
        protected_envelope["protected_payload"]["algorithm"],
        "AES-256-GCM"
    );
    assert_eq!(
        protected_envelope["protected_payload"]["key_reference"],
        "p05-canonical-payload-key"
    );
    assert!(!signed_payload.contains("\"a\":[true"));
    assert!(!signed_payload.contains("\"z\":1"));
    let stored_payload: String =
        sqlx::query_scalar("SELECT payload_json::text FROM event_store WHERE sequence = $1")
            .bind(success.first_event_sequence)
            .fetch_one(&primary)
            .await
            .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&stored_payload).unwrap(),
        protected_envelope
    );
    let outbox_payload: String =
        sqlx::query_scalar("SELECT payload_json::text FROM event_outbox WHERE event_sequence = $1")
            .bind(success.first_event_sequence)
            .fetch_one(&primary)
            .await
            .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&outbox_payload).unwrap(),
        serde_json::from_str::<serde_json::Value>(&stored_payload).unwrap()
    );
    assert!(!outbox_payload.contains("\"a\":[true"));
    let encrypted_columns = sqlx::query(
        "SELECT payload_ciphertext, payload_key_reference, payload_nonce \
         FROM event_store WHERE sequence = $1",
    )
    .bind(success.first_event_sequence)
    .fetch_one(&primary)
    .await
    .unwrap();
    let event_ciphertext: Vec<u8> = encrypted_columns.get("payload_ciphertext");
    let event_key_reference: String = encrypted_columns.get("payload_key_reference");
    let event_nonce: Vec<u8> = encrypted_columns.get("payload_nonce");
    assert!(event_ciphertext.len() >= 16);
    assert_eq!(event_key_reference, "p05-canonical-payload-key");
    assert_eq!(event_nonce.len(), 12);
    let outbox_encrypted_columns = sqlx::query(
        "SELECT payload_ciphertext, payload_key_reference, payload_nonce, visibility_subject \
         FROM event_outbox WHERE event_sequence = $1",
    )
    .bind(success.first_event_sequence)
    .fetch_one(&primary)
    .await
    .unwrap();
    assert_eq!(
        outbox_encrypted_columns.get::<Vec<u8>, _>("payload_ciphertext"),
        event_ciphertext
    );
    assert_eq!(
        outbox_encrypted_columns.get::<String, _>("payload_key_reference"),
        event_key_reference
    );
    assert_eq!(
        outbox_encrypted_columns.get::<Vec<u8>, _>("payload_nonce"),
        event_nonce
    );
    assert_eq!(
        outbox_encrypted_columns.get::<String, _>("visibility_subject"),
        success_draft.visibility_subject
    );
    let replay = store
        .load_replay_page("campaign_atomic_commit", 0, 10)
        .await
        .unwrap();
    assert_eq!(replay.len(), 2);
    assert_eq!(
        replay[0].payload,
        serde_json::json!({"a": [true, null], "z": 1})
    );

    // Fact promotion consumes the real encrypted, HMAC-verified canonical
    // event and external witness chain. It cannot substitute the process-local
    // EventStore fixture used by domain-only tests.
    let mut fact_draft = draft("persisted_fact_evidence", 0, &["DecisionCommitted"]);
    bind_campaign(&mut fact_draft, "campaign_persisted_fact_evidence");
    fact_draft.events[0].payload_json = serde_json::json!({
        "kind": "RecordDecision",
        "fact_source": "DecisionRecord",
        "target_fact_id": "persisted_fact_001"
    })
    .to_string();
    let fact_commit = store.commit(&fact_draft).await.unwrap();
    let evidence = store
        .load_committed_fact_evidence(
            "campaign_persisted_fact_evidence",
            fact_commit.first_event_sequence,
            "persisted_fact_001",
        )
        .await
        .unwrap();
    assert_eq!(
        evidence.event_sequence(),
        fact_commit.first_event_sequence as u64
    );
    assert_eq!(evidence.source(), FactSource::DecisionRecord);
    assert_eq!(evidence.target_fact_id().as_str(), "persisted_fact_001");
    assert_eq!(
        evidence.stream_id().as_str(),
        "campaign_persisted_fact_evidence"
    );

    // A database-side rejection after the first event proves that events,
    // outbox rows, the audit record, and the formal-commit marker roll back as
    // one primary transaction. The independent PREPARED witness is reconciled
    // to ABORTED rather than being silently erased.
    let event_count_before_rollback = scalar(&primary, "SELECT count(*) FROM event_store").await;
    let outbox_count_before_rollback = scalar(&primary, "SELECT count(*) FROM event_outbox").await;
    let audit_count_before_rollback =
        scalar(&primary, "SELECT count(*) FROM canonical_audit_log").await;
    let formal_count_before_rollback =
        scalar(&primary, "SELECT count(*) FROM formal_commits").await;
    sqlx::raw_sql(
        r#"
        CREATE OR REPLACE FUNCTION reject_atomicity_probe()
        RETURNS trigger LANGUAGE plpgsql AS $$
        BEGIN
            IF NEW.event_type = 'RejectForAtomicityProbe' THEN
                RAISE EXCEPTION 'atomicity probe rejection';
            END IF;
            RETURN NEW;
        END;
        $$;
        DROP TRIGGER IF EXISTS reject_atomicity_probe ON event_store;
        CREATE TRIGGER reject_atomicity_probe
        BEFORE INSERT ON event_store
        FOR EACH ROW EXECUTE FUNCTION reject_atomicity_probe();
        "#,
    )
    .execute(&primary)
    .await
    .unwrap();

    let failed = store
        .commit(&draft(
            "rollback",
            2,
            &["ClueDiscovered", "RejectForAtomicityProbe"],
        ))
        .await;
    assert!(matches!(
        failed,
        Err(CanonicalStoreError::PrimaryWrite { .. })
    ));

    sqlx::raw_sql(
        r#"
        DROP TRIGGER IF EXISTS reject_atomicity_probe ON event_store;
        DROP FUNCTION IF EXISTS reject_atomicity_probe();
        "#,
    )
    .execute(&primary)
    .await
    .unwrap();

    assert_eq!(
        scalar(&primary, "SELECT count(*) FROM event_store").await,
        event_count_before_rollback
    );
    assert_eq!(
        scalar(&primary, "SELECT count(*) FROM event_outbox").await,
        outbox_count_before_rollback
    );
    assert_eq!(
        scalar(&primary, "SELECT count(*) FROM canonical_audit_log").await,
        audit_count_before_rollback
    );
    assert_eq!(
        scalar(&primary, "SELECT count(*) FROM formal_commits").await,
        formal_count_before_rollback
    );

    let recovered = store.recover().await.unwrap();
    assert_eq!(
        recovered,
        RecoveryReport {
            finalized: 0,
            aborted: 1,
        }
    );
    assert_eq!(
        scalar(
            &witness,
            "SELECT count(*) FROM external_audit_witness WHERE commit_id = 'rollback' AND phase = 'ABORTED'"
        )
        .await,
        1
    );
    include!("02_failure_atomicity_and_witness.rs");
}
