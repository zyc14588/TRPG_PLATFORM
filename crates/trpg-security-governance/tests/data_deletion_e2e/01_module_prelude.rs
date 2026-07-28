use std::fs;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use sqlx::postgres::PgPoolOptions;
use sqlx::Row;
use trpg_data_eventing::cache_redis_impl::{ProjectionCacheEntry, RedisProjectionCache};
use trpg_data_eventing::event_bus_nats_impl::JetStreamOutboxPublisher;
use trpg_data_eventing::event_store_sqlx_outbox_projection::{
    AtomicCommitDraft, CanonicalEventDraft, PolicyAuditDraft, PostgresCanonicalStore,
};
use trpg_data_eventing::postgre_sql_sq_lx_pgvector::PostgresRagSnapshotRepository;
use trpg_data_eventing::rag_snapshot::RagSnapshotChunkDraft;
use trpg_security_governance::security_privacy::{
    BackupKeyDeletionSurface, DeletionBatchProgress, DeletionEvidenceStatus, DeletionJobStatus,
    DeletionRequestEvidence, DeletionSurface, DeletionTarget, DeletionTargetStatus, DeletionWorker,
    FilesystemDeletionSurface, NatsQueueDeletionSurface, PostgresDeletionRepository,
    PostgresLegalHoldResolver, PostgresRecordDeletionSurface, PrivacyError,
    RedisCacheDeletionSurface, S3ObjectDeletionSurface, MAX_DELETION_LEASE_RECOVERIES,
    REQUIRED_DELETION_TARGETS,
};
use trpg_shared_kernel::EventActorOriginWire;

const INTEGRITY_KEY: [u8; 32] = [0x73; 32];
const PAYLOAD_KEY: [u8; 32] = [0x84; 32];
const CACHE_KEY: [u8; 32] = [0x95; 32];
const INTEGRITY_KEY_ID: &str = "p05-deletion-integrity-key";
const PAYLOAD_KEY_ID: &str = "p05-deletion-master-payload-key";

struct SimulatedLeaseLossSurface {
    pool: sqlx::PgPool,
}

#[async_trait::async_trait]
impl DeletionSurface for SimulatedLeaseLossSurface {
    fn target(&self) -> DeletionTarget {
        DeletionTarget::Database
    }

    async fn delete_subject_batch(
        &self,
        subject_id: &str,
        _cursor: u64,
    ) -> Result<DeletionBatchProgress, PrivacyError> {
        let mut transaction = self.pool.begin().await.unwrap();
        sqlx::query(
            "UPDATE privacy_deletion_jobs SET status = 'failed', \
             failure_code = 'SIMULATED_LEASE_LOSS', lease_expires_at = NULL \
             WHERE subject_id = $1 AND status = 'running'",
        )
        .bind(subject_id)
        .execute(&mut *transaction)
        .await
        .unwrap();
        sqlx::query(
            "UPDATE privacy_subject_deletion_fences SET status = 'failed', \
             lease_expires_at = NULL WHERE subject_id = $1 AND status = 'running'",
        )
        .bind(subject_id)
        .execute(&mut *transaction)
        .await
        .unwrap();
        transaction.commit().await.unwrap();
        Err(PrivacyError::Storage)
    }

    async fn verify_absent(&self, _subject_id: &str) -> Result<bool, PrivacyError> {
        Ok(false)
    }
}

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
            visibility: None,
            projection_targets: Vec::new(),
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
            visibility: None,
            projection_targets: Vec::new(),
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
