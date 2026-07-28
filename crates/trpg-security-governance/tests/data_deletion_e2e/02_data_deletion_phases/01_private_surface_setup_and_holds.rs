{
    let database_url = std::env::var("P05_DATABASE_URL")
        .expect("P05_DATABASE_URL must point to the P05 PostgreSQL test database");
    let witness_url = std::env::var("P05_WITNESS_DATABASE_URL")
        .expect("P05_WITNESS_DATABASE_URL must point to an independent P05 witness database");
    let redis_url = std::env::var("P05_REDIS_URL")
        .expect("P05_REDIS_URL must point to the P05 Redis test service");
    let nats_url = std::env::var("P05_NATS_URL")
        .expect("P05_NATS_URL must point to the P05 NATS JetStream test service");
    let object_endpoint = std::env::var("P05_MINIO_ENDPOINT")
        .expect("P05_MINIO_ENDPOINT must point to the P05 S3-compatible object store");
    let object_region = std::env::var("P05_MINIO_REGION").expect("P05_MINIO_REGION is required");
    let object_bucket = std::env::var("P05_MINIO_BUCKET").expect("P05_MINIO_BUCKET is required");
    let object_access_key =
        std::env::var("P05_MINIO_ACCESS_KEY").expect("P05_MINIO_ACCESS_KEY is required");
    let object_secret_key =
        std::env::var("P05_MINIO_SECRET_KEY").expect("P05_MINIO_SECRET_KEY is required");
    let store = PostgresCanonicalStore::connect(
        &database_url,
        &witness_url,
        INTEGRITY_KEY_ID,
        &INTEGRITY_KEY,
        PAYLOAD_KEY_ID,
        &PAYLOAD_KEY,
    )
    .await
    .expect("connect primary and independent witness stores");
    store
        .prepare_for_service()
        .await
        .expect("apply canonical and privacy migrations");
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .expect("connect P05 PostgreSQL");
    let repository = PostgresDeletionRepository::new(pool.clone());
    repository.migrate().await.expect("apply privacy schema");

    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let subject_id = format!("subject_{nonce}");
    let job_id = format!("delete_{nonce}");
    let hold_reference = format!("hold_{nonce}");
    let root = tempfile::Builder::new()
        .prefix("p05-deletion-e2e-")
        .tempdir()
        .unwrap();
    let export_root = root.path().join("exports");
    fs::create_dir_all(export_root.join(&subject_id)).unwrap();
    fs::write(
        export_root.join(&subject_id).join("export.bin"),
        b"protected-export-ciphertext",
    )
    .unwrap();

    sqlx::query(
        "INSERT INTO users (user_id, login_normalized, password_hash, global_role) \
         VALUES ($1, $2, $3, 'USER')",
    )
    .bind(&subject_id)
    .bind(format!("player_{nonce}"))
    .bind(format!("argon2_test_hash_{nonce}"))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO sessions (session_id, user_id, token_hash, issued_at, expires_at) \
         VALUES ($1, $2, $3, now(), now() + interval '1 hour')",
    )
    .bind(format!("subject_session_{nonce}"))
    .bind(&subject_id)
    .bind(nonce.to_be_bytes().to_vec())
    .execute(&pool)
    .await
    .unwrap();

    let event_draft = private_event_draft(nonce, &subject_id);
    let persisted = store
        .commit(&event_draft)
        .await
        .expect("commit real subject-scoped canonical event");
    let rag_content = format!("private-rag-memory-{nonce}");
    let snapshot_id = format!("privacy_snapshot_{nonce}");
    let chunk_id = format!("privacy_chunk_{nonce}");
    let mut rag_chunk = RagSnapshotChunkDraft {
        chunk_id: chunk_id.clone(),
        source_event_sequence: persisted.first_event_sequence,
        derivation_event_sequence: 0,
        source_type: "memory_event".to_owned(),
        copyright_status: "original".to_owned(),
        allowed_use: "private_retrieval".to_owned(),
        content: rag_content.clone(),
        embedding_model: "p05-test-embedding".to_owned(),
        embedding: vec![0.25, 0.5, 0.75],
    };
    let mut derivation_draft = event_draft.clone();
    derivation_draft.commit_id = format!("privacy_rag_derivation_{nonce}");
    derivation_draft.idempotency_key = format!("privacy_rag_derivation_idempotency_{nonce}");
    derivation_draft.command_id = format!("privacy_rag_derivation_command_{nonce}");
    derivation_draft.expected_version = 1;
    derivation_draft.correlation_id = format!("privacy_rag_derivation_correlation_{nonce}");
    derivation_draft.causation_id = format!("privacy_rag_derivation_causation_{nonce}");
    derivation_draft.trace_id = format!("privacy_rag_derivation_trace_{nonce}");
    derivation_draft.audit.openfga_decision_id = format!("privacy_rag_fga_{nonce}");
    derivation_draft.audit.opa_decision_id = format!("privacy_rag_opa_{nonce}");
    derivation_draft.events = vec![CanonicalEventDraft {
        event_type: "RagChunkDerived".to_owned(),
        payload_json: serde_json::json!({
            "source_event_sequence": persisted.first_event_sequence,
            "snapshot_id": snapshot_id.clone(),
            "chunk_id": chunk_id.clone(),
            "content_hash": rag_chunk.content_hash(),
            "source_type": rag_chunk.source_type,
            "copyright_status": rag_chunk.copyright_status,
            "allowed_use": rag_chunk.allowed_use,
            "embedding_model": rag_chunk.embedding_model,
            "embedding_dimensions": rag_chunk.embedding.len(),
            "embedding_hash": rag_chunk.embedding_hash(),
        })
        .to_string(),
        visibility: None,
        projection_targets: Vec::new(),
    }];
    let derivation = store
        .commit(&derivation_draft)
        .await
        .expect("commit formal RAG derivation evidence");
    rag_chunk.derivation_event_sequence = derivation.first_event_sequence;
    PostgresRagSnapshotRepository::new(pool.clone())
        .replace_snapshot(&event_draft.campaign_id, &snapshot_id, &[rag_chunk])
        .await
        .expect("index actual pgvector RAG read model");

    let database =
        PostgresRecordDeletionSurface::new(pool.clone(), DeletionTarget::Database).unwrap();
    let rag = PostgresRecordDeletionSurface::new(pool.clone(), DeletionTarget::RagIndex).unwrap();
    let cache_key = format!("privacy_projection_{nonce}");
    let production_cache = RedisProjectionCache::connect(
        &redis_url,
        "trpg:realtime:projection",
        "p05-privacy-cache-key",
        &CACHE_KEY,
    )
    .await
    .unwrap();
    production_cache
        .put(
            &ProjectionCacheEntry::new(
                &cache_key,
                &event_draft.campaign_id,
                &subject_id,
                1,
                "private_to_player",
                &subject_id,
                "human_keeper_statement",
                format!("privacy_fact_{nonce}"),
                format!(r#"{{"private_cache":"cache-secret-{nonce}"}}"#),
                300,
            )
            .unwrap(),
        )
        .await
        .unwrap();
    let cache = RedisCacheDeletionSurface::connect(&redis_url, "trpg:realtime:projection")
        .await
        .unwrap();
    cache
        .remove_subject_index_for_test(&subject_id)
        .await
        .unwrap();
    assert!(
        !cache.verify_absent(&subject_id).await.unwrap(),
        "an orphaned resident cache entry must not be mistaken for absence"
    );
    let object_storage = S3ObjectDeletionSurface::connect(
        &object_endpoint,
        &object_region,
        &object_bucket,
        &object_access_key,
        &object_secret_key,
    )
    .await
    .expect("connect real S3-compatible object deletion surface");
    object_storage
        .put_protected_object(
            &subject_id,
            &format!("object_{nonce}_bin"),
            b"protected-object-ciphertext",
        )
        .await
        .expect("put subject-scoped protected object");
    let publisher = JetStreamOutboxPublisher::connect(
        store.clone(),
        &nats_url,
        &format!("p05_deletion_publisher_{nonce}"),
        None,
    )
    .await
    .unwrap();
    publisher.ensure_stream().await.unwrap();
    publisher.publish_batch().await.unwrap();
    let published: bool = sqlx::query_scalar(
        "SELECT published_at IS NOT NULL FROM event_outbox \
         WHERE commit_id = $1 AND data_subject_id = $2",
    )
    .bind(&event_draft.commit_id)
    .bind(&subject_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        published,
        "the actual canonical outbox row must reach JetStream"
    );
    let queue = NatsQueueDeletionSurface::connect_crypto_erasure(
        &nats_url,
        "TRPG_CANONICAL_EVENTS",
        pool.clone(),
    )
    .await
    .unwrap();
    for batch_probe in 0..129_u16 {
        queue
            .put_canonical_for_test(
                &subject_id,
                format!(r#"{{"protected_payload":"batch-probe-{batch_probe}"}}"#).as_bytes(),
            )
            .await
            .unwrap();
    }
    let backup_key = BackupKeyDeletionSurface::new(pool.clone());

    let legal_holds = PostgresLegalHoldResolver::new(pool.clone());
    legal_holds
        .set_hold(&subject_id, &hold_reference, true)
        .await
        .unwrap();
    assert_eq!(
        repository
            .request(
                &job_id,
                &subject_id,
                "privacy_officer",
                "user_erasure_v1",
                &deletion_evidence(nonce),
            )
            .await
            .unwrap_err(),
        PrivacyError::LegacyTwoPhaseDisabled
    );
    assert_eq!(
        repository.load(&job_id).await.unwrap_err(),
        PrivacyError::JobNotFound
    );

    let worker = DeletionWorker::new(
        repository.clone(),
        Arc::new(legal_holds.clone()),
        vec![
            Box::new(database),
            Box::new(rag),
            Box::new(object_storage.clone()),
            Box::new(cache.clone()),
            Box::new(queue.clone()),
            Box::new(FilesystemDeletionSurface::new(&export_root, DeletionTarget::Export).unwrap()),
            Box::new(backup_key),
        ],
    )
    .unwrap();

    assert!(!object_storage.verify_absent(&subject_id).await.unwrap());
    let unrelated_event_hash: String =
        sqlx::query_scalar("SELECT event_integrity_hash FROM event_store WHERE sequence = $1")
            .bind(persisted.first_event_sequence)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        repository
            .record_confirmed(
                &job_id,
                &subject_id,
                "privacy_officer",
                "user_erasure_v1",
                &deletion_evidence(nonce),
                u64::try_from(persisted.first_event_sequence).unwrap(),
                &unrelated_event_hash,
            )
            .await
            .expect_err("an unrelated canonical event cannot confirm deletion evidence"),
        PrivacyError::DeletionEvidenceMismatch
    );
    let (deletion_event_sequence, deletion_event_hash) =
        commit_deletion_request(&store, &pool, nonce, &subject_id, &job_id).await;
    let confirmed = repository
        .record_confirmed(
            &job_id,
            &subject_id,
            "privacy_officer",
            "user_erasure_v1",
            &deletion_evidence(nonce),
            deletion_event_sequence,
            &deletion_event_hash,
        )
        .await
        .unwrap();
    assert_eq!(confirmed.status, DeletionJobStatus::Requested);
    assert_eq!(confirmed.evidence_status, DeletionEvidenceStatus::Confirmed);
    assert_eq!(confirmed.targets.len(), REQUIRED_DELETION_TARGETS.len());
    publisher
        .publish_batch()
        .await
        .expect("publish canonical deletion request before execution");

    let blocked = worker.execute(&job_id).await.unwrap();
    assert_eq!(blocked.status, DeletionJobStatus::BlockedLegalHold);
    assert!(!object_storage.verify_absent(&subject_id).await.unwrap());
    let rag_rows_before_release: i64 =
        sqlx::query_scalar("SELECT count(*) FROM rag_snapshot_chunk WHERE visibility_subject = $1")
            .bind(&subject_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(rag_rows_before_release, 1);
    assert!(!cache.verify_absent(&subject_id).await.unwrap());
    assert!(!queue.verify_absent(&subject_id).await.unwrap());

    legal_holds
        .set_hold(&subject_id, &hold_reference, false)
        .await
        .unwrap();
    let completed = worker.execute(&job_id).await.unwrap();
    assert_eq!(completed.status, DeletionJobStatus::Completed);
    assert!(completed.all_targets_verified());
    assert!(completed
        .targets
        .iter()
        .all(|target| target.status == DeletionTargetStatus::Verified));
    include!("02_deletion_progress_and_surface_verification.rs");
}
