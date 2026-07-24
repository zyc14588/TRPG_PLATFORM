use std::fs;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::postgres::PgPoolOptions;
use sqlx::Row;
use trpg_data_eventing::cache_redis_impl::{ProjectionCacheEntry, RedisProjectionCache};
use trpg_data_eventing::event_bus_nats_impl::JetStreamOutboxPublisher;
use trpg_data_eventing::event_store_sqlx_outbox_projection::{
    AtomicCommitDraft, CanonicalEventDraft, PolicyAuditDraft, PostgresCanonicalStore,
};
use trpg_data_eventing::postgre_sql_sq_lx_pgvector::PostgresRagSnapshotRepository;
use trpg_data_eventing::rag_snapshot::RagSnapshotChunkDraft;
use trpg_privacy::{
    BackupKeyDeletionSurface, DeletionEvidenceStatus, DeletionJobStatus, DeletionRequestEvidence,
    DeletionSurface, DeletionTarget, DeletionTargetStatus, DeletionWorker,
    FilesystemDeletionSurface, NatsQueueDeletionSurface, PostgresDeletionRepository,
    PostgresLegalHoldResolver, PostgresRecordDeletionSurface, PrivacyError,
    RedisCacheDeletionSurface, S3ObjectDeletionSurface, REQUIRED_DELETION_TARGETS,
};
use trpg_shared_kernel::EventActorOriginWire;

const INTEGRITY_KEY: [u8; 32] = [0x73; 32];
const PAYLOAD_KEY: [u8; 32] = [0x84; 32];
const CACHE_KEY: [u8; 32] = [0x95; 32];
const INTEGRITY_KEY_ID: &str = "p05-deletion-integrity-key";
const PAYLOAD_KEY_ID: &str = "p05-deletion-master-payload-key";

fn deletion_evidence(nonce: u128) -> DeletionRequestEvidence {
    DeletionRequestEvidence::new(
        format!("privacy_campaign_{nonce}"),
        format!("command_{nonce}"),
        format!("correlation_{nonce}"),
        format!("causation_{nonce}"),
        "platform.security_privacy_copyright.data_deletion_requested",
    )
    .unwrap()
}

fn private_event_draft(nonce: u128, subject_id: &str) -> AtomicCommitDraft {
    let campaign_id = format!("privacy_campaign_{nonce}");
    let stream_id = format!("privacy_stream_{nonce}");
    let commit_id = format!("privacy_commit_{nonce}");
    AtomicCommitDraft {
        commit_id: commit_id.clone(),
        campaign_id: campaign_id.clone(),
        stream_id: stream_id.clone(),
        idempotency_key: format!("privacy_idempotency_{nonce}"),
        expected_version: 0,
        command_id: format!("privacy_command_{nonce}"),
        authenticated_actor_id: "privacy_workflow".to_owned(),
        authenticated_actor_role: "workflow".to_owned(),
        authenticated_actor_origin: EventActorOriginWire::Workload {
            role: "workflow_engine".to_owned(),
        },
        authority_mode: "human_kp".to_owned(),
        authority_contract_version: 1,
        authority_contract_id: format!("privacy_authority_{nonce}"),
        authority_owner: "privacy_keeper".to_owned(),
        visibility_label: "private_to_player".to_owned(),
        visibility_subject: subject_id.to_owned(),
        data_subject_id: subject_id.to_owned(),
        provenance_kind: "human_keeper_statement".to_owned(),
        provenance_reference: format!("privacy_fact_{nonce}"),
        provenance_recorded_by: "privacy_keeper".to_owned(),
        correlation_id: format!("privacy_correlation_{nonce}"),
        causation_id: format!("privacy_causation_{nonce}"),
        trace_id: format!("privacy_trace_{nonce}"),
        events: vec![CanonicalEventDraft {
            event_type: "PrivatePlayerMemoryRecorded".to_owned(),
            payload_json: format!(r#"{{"private_memory":"deletion-secret-{nonce}"}}"#),
        }],
        audit: PolicyAuditDraft {
            actor_id: "privacy_officer".to_owned(),
            actor_origin: "user_session".to_owned(),
            authentication_reference: format!("privacy_session_{nonce}"),
            resource_type: "player_memory".to_owned(),
            resource_id: stream_id,
            action: "write_official_state".to_owned(),
            requested_role: "human_keeper".to_owned(),
            openfga_decision_id: format!("privacy_fga_{nonce}"),
            openfga_policy_revision: "privacy_fga_model_v1".to_owned(),
            opa_decision_id: format!("privacy_opa_{nonce}"),
            opa_policy_revision: "privacy_opa_bundle_v1".to_owned(),
        },
    }
}

fn deletion_request_draft(nonce: u128, subject_id: &str, job_id: &str) -> AtomicCommitDraft {
    let campaign_id = format!("privacy_campaign_{nonce}");
    AtomicCommitDraft {
        commit_id: format!("privacy_deletion_commit_{nonce}"),
        campaign_id,
        stream_id: subject_id.to_owned(),
        idempotency_key: format!("privacy_deletion_idempotency_{nonce}"),
        expected_version: 0,
        command_id: format!("command_{nonce}"),
        authenticated_actor_id: "privacy_workflow".to_owned(),
        authenticated_actor_role: "workflow".to_owned(),
        authenticated_actor_origin: EventActorOriginWire::Workload {
            role: "workflow_engine".to_owned(),
        },
        authority_mode: "human_kp".to_owned(),
        authority_contract_version: 1,
        authority_contract_id: format!("privacy_authority_{nonce}"),
        authority_owner: "privacy_keeper".to_owned(),
        visibility_label: "private_to_player".to_owned(),
        visibility_subject: subject_id.to_owned(),
        data_subject_id: subject_id.to_owned(),
        provenance_kind: "user_statement".to_owned(),
        provenance_reference: format!("command_{nonce}"),
        provenance_recorded_by: "privacy_officer".to_owned(),
        correlation_id: format!("correlation_{nonce}"),
        causation_id: format!("causation_{nonce}"),
        trace_id: format!("privacy_deletion_trace_{nonce}"),
        events: vec![CanonicalEventDraft {
            event_type: "platform.security_privacy_copyright.data_deletion_requested".to_owned(),
            payload_json: serde_json::json!({
                "DataDeletionRequested": {
                    "job_id": job_id,
                    "subject_id": subject_id,
                    "requested_by": "privacy_officer",
                    "retention_policy": "user_erasure_v1",
                    "reason": "[redacted]",
                }
            })
            .to_string(),
        }],
        audit: PolicyAuditDraft {
            actor_id: "privacy_officer".to_owned(),
            actor_origin: "user_session".to_owned(),
            authentication_reference: format!("privacy_session_{nonce}"),
            resource_type: "data_subject".to_owned(),
            resource_id: subject_id.to_owned(),
            action: "delete_personal_data".to_owned(),
            requested_role: "privacy_deletion_requester".to_owned(),
            openfga_decision_id: format!("privacy_deletion_fga_{nonce}"),
            openfga_policy_revision: "privacy_fga_model_v1".to_owned(),
            opa_decision_id: format!("privacy_deletion_opa_{nonce}"),
            opa_policy_revision: "privacy_opa_bundle_v1".to_owned(),
        },
    }
}

async fn commit_deletion_request(
    store: &PostgresCanonicalStore,
    pool: &sqlx::PgPool,
    nonce: u128,
    subject_id: &str,
    job_id: &str,
) -> (u64, String) {
    let persisted = store
        .commit(&deletion_request_draft(nonce, subject_id, job_id))
        .await
        .expect("commit formal deletion request event");
    let (sequence, integrity_hash): (i64, String) = sqlx::query_as(
        "SELECT sequence, event_integrity_hash FROM event_store WHERE sequence = $1",
    )
    .bind(persisted.first_event_sequence)
    .fetch_one(pool)
    .await
    .expect("load exact canonical deletion event evidence");
    (u64::try_from(sequence).unwrap(), integrity_hash)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn data_deletion_persists_blocks_on_hold_and_verifies_every_real_surface() {
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
    let root = std::env::temp_dir().join(format!("p05-deletion-e2e-{nonce}"));
    let export_root = root.join("exports");
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
        PrivacyError::Database
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

    // Re-open through a fresh repository handle to prove status is durable,
    // then independently query every backing store instead of trusting Worker
    // return values.
    let reloaded = PostgresDeletionRepository::new(pool.clone())
        .load(&job_id)
        .await
        .unwrap();
    assert_eq!(reloaded.status, DeletionJobStatus::Completed);
    assert!(reloaded.all_targets_verified());
    let remaining_rag_records: i64 =
        sqlx::query_scalar("SELECT count(*) FROM rag_snapshot_chunk WHERE visibility_subject = $1")
            .bind(&subject_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(remaining_rag_records, 0);
    assert!(cache.verify_absent(&subject_id).await.unwrap());
    assert!(queue.verify_absent(&subject_id).await.unwrap());
    assert!(object_storage.verify_absent(&subject_id).await.unwrap());
    assert!(!export_root.join(&subject_id).exists());
    let key_row = sqlx::query(
        "SELECT wrapped_key, destroyed_at IS NOT NULL AS destroyed \
         FROM privacy_subject_keys WHERE subject_id = $1",
    )
    .bind(&subject_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let wrapped_key: Option<Vec<u8>> = key_row.try_get("wrapped_key").unwrap();
    let destroyed: bool = key_row.try_get("destroyed").unwrap();
    assert!(wrapped_key.is_none());
    assert!(destroyed);
    assert!(store
        .load_replay_page(&event_draft.campaign_id, 0, 10)
        .await
        .unwrap()
        .is_empty());
    let erased_identity = sqlx::query(
        "SELECT login_normalized, password_hash, disabled_at IS NOT NULL AS disabled \
         FROM users WHERE user_id = $1",
    )
    .bind(&subject_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(erased_identity
        .get::<String, _>("login_normalized")
        .starts_with("deleted_"));
    assert!(erased_identity
        .get::<String, _>("password_hash")
        .starts_with("DELETED_ACCOUNT_NO_LOGIN_"));
    assert!(erased_identity.get::<bool, _>("disabled"));
    let session_count: i64 = sqlx::query_scalar("SELECT count(*) FROM sessions WHERE user_id = $1")
        .bind(&subject_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(session_count, 0);
    let protected_event: serde_json::Value =
        sqlx::query_scalar("SELECT payload_json FROM event_store WHERE sequence = $1")
            .bind(persisted.first_event_sequence)
            .fetch_one(&pool)
            .await
            .unwrap();
    let protected_wire = serde_json::to_string(&protected_event).unwrap();
    assert!(protected_wire.contains("protected_payload"));
    assert!(!protected_wire.contains("deletion-secret"));

    // Completion is idempotent and does not recreate or reclassify data.
    assert_eq!(
        worker.execute(&job_id).await.unwrap().status,
        DeletionJobStatus::Completed
    );

    fs::remove_dir_all(root).unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn deletion_cannot_complete_when_a_required_surface_is_missing() {
    let database_url = std::env::var("P05_DATABASE_URL")
        .expect("P05_DATABASE_URL must point to the P05 PostgreSQL test database");
    let witness_url = std::env::var("P05_WITNESS_DATABASE_URL")
        .expect("P05_WITNESS_DATABASE_URL must point to an independent P05 witness database");
    let store = PostgresCanonicalStore::connect(
        &database_url,
        &witness_url,
        INTEGRITY_KEY_ID,
        &INTEGRITY_KEY,
        PAYLOAD_KEY_ID,
        &PAYLOAD_KEY,
    )
    .await
    .expect("connect canonical store for missing-surface evidence");
    store.prepare_for_service().await.unwrap();
    let pool = PgPoolOptions::new()
        .max_connections(3)
        .connect(&database_url)
        .await
        .expect("connect P05 PostgreSQL");
    let repository = PostgresDeletionRepository::new(pool.clone());
    repository.migrate().await.unwrap();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let job_id = format!("missing_surface_{nonce}");
    let subject_id = format!("missing_subject_{nonce}");
    let (event_sequence, event_hash) =
        commit_deletion_request(&store, repository.pool(), nonce, &subject_id, &job_id).await;
    repository
        .record_confirmed(
            &job_id,
            &subject_id,
            "privacy_officer",
            "user_erasure_v1",
            &deletion_evidence(nonce),
            event_sequence,
            &event_hash,
        )
        .await
        .unwrap();
    let legal_holds = PostgresLegalHoldResolver::new(pool);
    let worker =
        DeletionWorker::new(repository.clone(), Arc::new(legal_holds), Vec::new()).unwrap();

    assert!(worker.execute(&job_id).await.is_err());
    let failed = repository.load(&job_id).await.unwrap();
    assert_eq!(failed.status, DeletionJobStatus::Failed);
    assert!(!failed.all_targets_verified());
}

#[tokio::test]
async fn filesystem_verification_does_not_misreport_io_failures_as_absence() {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let non_directory_root =
        std::env::temp_dir().join(format!("p05-deletion-verification-root-{nonce}"));
    fs::write(&non_directory_root, b"not-a-directory").unwrap();
    let surface =
        FilesystemDeletionSurface::new(&non_directory_root, DeletionTarget::ObjectStorage).unwrap();

    let error = surface
        .verify_absent("subject_with_unreadable_root")
        .await
        .expect_err("an I/O failure is not proof that subject data is absent");

    assert_eq!(error, PrivacyError::Storage);
    fs::remove_file(non_directory_root).unwrap();
}

#[tokio::test]
async fn retained_security_and_privacy_history_rejects_bulk_removal() {
    let database_url = std::env::var("P05_DATABASE_URL")
        .expect("P05_DATABASE_URL must point to the P05 PostgreSQL test database");
    let repository = PostgresDeletionRepository::connect(&database_url)
        .await
        .expect("connect P05 PostgreSQL");
    repository.migrate().await.expect("apply P05 migrations");

    for statement in [
        "TRUNCATE TABLE campaign_group_memberships",
        "TRUNCATE TABLE cloud_egress_audit",
        "TRUNCATE TABLE privacy_deletion_jobs CASCADE",
        "TRUNCATE TABLE privacy_subject_deletion_fences",
        "TRUNCATE TABLE privacy_erased_subjects",
        "TRUNCATE TABLE privacy_subject_keys",
        "TRUNCATE TABLE privacy_legal_holds",
        "DELETE FROM privacy_deletion_jobs WHERE false",
        "DELETE FROM privacy_deletion_job_targets WHERE false",
        "DELETE FROM privacy_subject_deletion_fences WHERE false",
        "DELETE FROM privacy_erased_subjects WHERE false",
        "DELETE FROM privacy_subject_keys WHERE false",
        "DELETE FROM privacy_legal_holds WHERE false",
    ] {
        assert!(
            sqlx::query(statement)
                .execute(repository.pool())
                .await
                .is_err(),
            "retained security history unexpectedly accepted: {statement}"
        );
    }
}
