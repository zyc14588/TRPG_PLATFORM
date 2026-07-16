use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::FromRow;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, FromRow)]
pub struct EventStoreRecord {
    pub sequence: i64,
    pub event_type: String,
    pub command_id: String,
    pub idempotency_key: String,
    pub expected_version: i64,
    pub authority_mode: String,
    pub authority_contract_version: i64,
    pub visibility_label: String,
    pub fact_provenance_kind: String,
    pub fact_provenance_reference: String,
    pub fact_recorded_by: String,
    pub correlation_id: String,
    pub causation_id: String,
    pub payload_json: Value,
    pub recorded_at: DateTime<Utc>,
    pub campaign_id: String,
    pub stream_version: i64,
    pub authenticated_actor_id: String,
    pub resource_type: String,
    pub resource_id: String,
    pub authority_contract_id: String,
    pub authority_owner: String,
    pub visibility_subject: String,
    pub trace_id: String,
    pub event_integrity_hash: Option<String>,
    pub stream_id: String,
    pub event_schema_version: i32,
    pub idempotency_operation: String,
    pub request_hash: String,
    pub request_hash_source: String,
    pub integrity_status: String,
    pub payload_integrity_source: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, FromRow)]
pub struct EventOutboxRecord {
    pub outbox_id: i64,
    pub event_id: i64,
    pub event_sequence: i64,
    pub nats_subject: String,
    pub idempotency_key: String,
    pub visibility_label: String,
    pub correlation_id: String,
    pub causation_id: String,
    pub payload_json: Value,
    pub published_at: Option<DateTime<Utc>>,
    pub retry_count: i32,
    pub commit_id: Option<String>,
    pub claimed_at: Option<DateTime<Utc>>,
    pub claim_owner: Option<String>,
    pub last_error: Option<String>,
    pub dead_lettered_at: Option<DateTime<Utc>>,
    pub campaign_id: String,
    pub stream_id: String,
    pub event_schema_version: i32,
    pub idempotency_operation: String,
    pub request_hash: String,
    pub request_hash_source: String,
    pub integrity_status: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, FromRow)]
pub struct ProjectionCheckpointRecord {
    pub projection_name: String,
    pub stream_id: String,
    pub version: i64,
    pub last_event_sequence: i64,
    pub projection_hash: String,
    pub rebuilt_at: DateTime<Utc>,
    pub campaign_id: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, FromRow)]
pub struct FormalCommitRecord {
    pub commit_id: String,
    pub campaign_id: String,
    pub idempotency_key: String,
    pub request_hash: String,
    pub expected_version: i64,
    pub first_event_sequence: i64,
    pub last_event_sequence: i64,
    pub first_stream_version: i64,
    pub last_stream_version: i64,
    pub audit_sequence: i64,
    pub witness_prepare_sequence: i64,
    pub witness_prepare_hash: String,
    pub committed_at: DateTime<Utc>,
    pub stream_id: String,
    pub idempotency_operation: String,
    pub status: String,
    pub result_event_sequence: i64,
    pub response_payload: Value,
}
