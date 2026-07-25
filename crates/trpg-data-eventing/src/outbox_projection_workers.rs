crate::define_data_event_module!(
    OutboxProjectionWorkersCommand,
    OutboxProjectionWorkersOperation,
    append_outbox_projection_workers_event,
    "outbox_projection_workers",
    "OutboxProjectionWorkerRecorded",
    "data_eventing.outbox_projection_workers.event_schema",
    crate::DataEventOperation::OutboxPublish,
    ["event_outbox", "projection_worker_checkpoint"]
);

use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Utc};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::types::Json;
use sqlx::{FromRow, PgPool, Row};
use trpg_shared_kernel::EventActorOriginWire;

use crate::event_store_projections::{
    CanonicalProjectionHasher, ProjectionHashError, PROJECTION_HASH_GENESIS,
};
use crate::event_store_sqlx_outbox_projection::CanonicalReplayEvent;

pub const EVENTING_COMMAND_TOTAL_METRIC: &str = "trpg_command_total";
const MAX_METRIC_OBSERVATIONS: usize = 1_024;

/// A bounded metric exemplar. Correlation and causation stay attached to the
/// observation used for diagnostics without becoming unbounded time-series
/// label values in a metrics backend.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventingMetricObservation {
    pub metric_name: &'static str,
    pub operation: &'static str,
    pub outcome: &'static str,
    pub value: u64,
    pub correlation_id: String,
    pub causation_id: String,
    pub visibility_label: String,
    pub provenance_kind: String,
}

#[derive(Default)]
pub struct EventingMetrics {
    counters: Mutex<HashMap<(&'static str, &'static str, &'static str), u64>>,
    observations: Mutex<VecDeque<EventingMetricObservation>>,
}

impl fmt::Debug for EventingMetrics {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EventingMetrics")
            .field("counters", &"[METRIC COUNTERS]")
            .field("observations", &"[BOUNDED METRIC EXEMPLARS]")
            .finish()
    }
}

impl EventingMetrics {
    pub fn record_outbox_publish(&self, claim: &OutboxClaim, outcome: &'static str) {
        let operation = "outbox_publish";
        let mut counters = self
            .counters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let counter = counters
            .entry((EVENTING_COMMAND_TOTAL_METRIC, operation, outcome))
            .or_default();
        *counter = counter.saturating_add(1);
        let value = *counter;
        drop(counters);

        let observation = EventingMetricObservation {
            metric_name: EVENTING_COMMAND_TOTAL_METRIC,
            operation,
            outcome,
            value,
            correlation_id: claim.correlation_id.clone(),
            causation_id: claim.causation_id.clone(),
            visibility_label: claim.visibility_label.clone(),
            provenance_kind: claim.provenance_kind.clone(),
        };
        tracing::info!(
            metric_name = observation.metric_name,
            operation = observation.operation,
            outcome = observation.outcome,
            value = observation.value,
            correlation_id = %observation.correlation_id,
            causation_id = %observation.causation_id,
            visibility_label = %observation.visibility_label,
            provenance_kind = %observation.provenance_kind,
            "eventing metric observation"
        );
        let mut observations = self
            .observations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if observations.len() == MAX_METRIC_OBSERVATIONS {
            observations.pop_front();
        }
        observations.push_back(observation);
    }

    pub fn counter_value(
        &self,
        metric_name: &'static str,
        operation: &'static str,
        outcome: &'static str,
    ) -> u64 {
        self.counters
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&(metric_name, operation, outcome))
            .copied()
            .unwrap_or_default()
    }

    pub fn observations(&self) -> Vec<EventingMetricObservation> {
        self.observations
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .cloned()
            .collect()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutboxLeasePolicy {
    pub lease_duration: Duration,
    pub initial_backoff: Duration,
    pub maximum_backoff: Duration,
    pub maximum_attempts: i32,
}

impl Default for OutboxLeasePolicy {
    fn default() -> Self {
        Self {
            lease_duration: Duration::from_secs(60),
            initial_backoff: Duration::from_secs(1),
            maximum_backoff: Duration::from_secs(15 * 60),
            maximum_attempts: 10,
        }
    }
}

impl OutboxLeasePolicy {
    fn validate(self) -> Result<Self, EventWorkerError> {
        if self.lease_duration.is_zero()
            || self.initial_backoff.is_zero()
            || self.maximum_backoff < self.initial_backoff
            || self.maximum_attempts <= 0
            || duration_milliseconds(self.lease_duration).is_none()
            || duration_milliseconds(self.initial_backoff).is_none()
            || duration_milliseconds(self.maximum_backoff).is_none()
        {
            return Err(EventWorkerError::Configuration(
                "invalid_outbox_lease_policy",
            ));
        }
        Ok(self)
    }

    fn backoff_for_attempt(self, attempt: i32) -> Duration {
        let exponent = u32::try_from(attempt.saturating_sub(1))
            .unwrap_or(0)
            .min(30);
        let multiplier = 1_u128 << exponent;
        let initial = self.initial_backoff.as_millis();
        let maximum = self.maximum_backoff.as_millis();
        let milliseconds = initial.saturating_mul(multiplier).min(maximum);
        Duration::from_millis(u64::try_from(milliseconds).unwrap_or(u64::MAX))
    }
}

#[derive(Clone, Debug, PartialEq, FromRow)]
pub struct OutboxClaim {
    pub outbox_id: i64,
    pub event_sequence: i64,
    pub subject: String,
    pub idempotency_key: String,
    pub visibility_label: String,
    pub correlation_id: String,
    pub causation_id: String,
    pub payload_json: Value,
    pub commit_id: Option<String>,
    pub event_type: String,
    pub event_schema_version: i32,
    pub campaign_id: String,
    pub stream_id: String,
    pub stream_version: i64,
    pub expected_version: i64,
    pub event_idempotency_key: String,
    pub idempotency_operation: String,
    pub authenticated_actor_id: String,
    pub authenticated_actor_role: String,
    pub authenticated_actor_origin: Json<EventActorOriginWire>,
    pub resource_type: String,
    pub resource_id: String,
    pub authority_contract_id: String,
    pub authority_owner: String,
    pub authority_contract_version: i64,
    pub command_id: String,
    pub visibility_subject: String,
    pub data_subject_id: String,
    pub provenance_kind: String,
    pub provenance_reference: String,
    pub provenance_recorded_by: String,
    pub trace_id: String,
    pub recorded_at: DateTime<Utc>,
    pub event_integrity_hash: Option<String>,
    pub request_hash: String,
    pub request_hash_source: String,
    pub integrity_status: String,
    pub payload_integrity_source: String,
    pub retry_count: i32,
    pub claimed_at: DateTime<Utc>,
    pub locked_until: DateTime<Utc>,
    pub claim_token: String,
}

impl OutboxClaim {
    pub fn validate_for_publish(&self) -> Result<(), EventWorkerError> {
        let valid = self.integrity_status == "verified_hmac"
            && self.request_hash_source == "formal_commit"
            && self.event_integrity_hash.as_deref().is_some_and(|hash| {
                hash.len() == 76
                    && hash.starts_with("hmac-sha256:")
                    && hash[12..].bytes().all(|byte| byte.is_ascii_hexdigit())
            })
            && self.commit_id.is_some()
            && self.payload_json.get("protected_payload").is_some()
            && serde_json::from_str::<Value>(&self.payload_integrity_source)
                .is_ok_and(|source| source == self.payload_json);
        let required = [
            self.subject.as_str(),
            self.idempotency_key.as_str(),
            self.event_idempotency_key.as_str(),
            self.event_type.as_str(),
            self.campaign_id.as_str(),
            self.stream_id.as_str(),
            self.idempotency_operation.as_str(),
            self.authenticated_actor_id.as_str(),
            self.authenticated_actor_role.as_str(),
            self.resource_type.as_str(),
            self.resource_id.as_str(),
            self.authority_contract_id.as_str(),
            self.authority_owner.as_str(),
            self.command_id.as_str(),
            self.visibility_label.as_str(),
            self.visibility_subject.as_str(),
            self.data_subject_id.as_str(),
            self.provenance_kind.as_str(),
            self.provenance_reference.as_str(),
            self.provenance_recorded_by.as_str(),
            self.correlation_id.as_str(),
            self.causation_id.as_str(),
            self.trace_id.as_str(),
            self.request_hash.as_str(),
            self.request_hash_source.as_str(),
            self.integrity_status.as_str(),
            self.payload_integrity_source.as_str(),
            self.claim_token.as_str(),
        ];
        if valid
            && required.iter().all(|value| !value.trim().is_empty())
            && actor_origin_is_complete(&self.authenticated_actor_origin.0)
            && self.subject == crate::NATS_EVENTS_APPENDED
            && self.event_sequence > 0
            && self.stream_version > 0
            && self.event_schema_version > 0
            && self.expected_version >= 0
            && self.authority_contract_version > 0
            && self.locked_until > self.claimed_at
        {
            Ok(())
        } else {
            Err(EventWorkerError::InvalidOutboxPayload)
        }
    }
}

fn actor_origin_is_complete(origin: &EventActorOriginWire) -> bool {
    match origin {
        EventActorOriginWire::UserSession { session_id } => !session_id.trim().is_empty(),
        EventActorOriginWire::Workload { role } => !role.trim().is_empty(),
        EventActorOriginWire::AgentRun {
            run_id,
            class,
            campaign_id,
        } => {
            !run_id.trim().is_empty() && !class.trim().is_empty() && !campaign_id.trim().is_empty()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutboxFailureCode {
    JetStreamPublishFailed,
    InvalidEnvelope,
    PublishAcknowledgementTimedOut,
}

impl OutboxFailureCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::JetStreamPublishFailed => "JETSTREAM_PUBLISH_FAILED",
            Self::InvalidEnvelope => "INVALID_OUTBOX_ENVELOPE",
            Self::PublishAcknowledgementTimedOut => "PUBLISH_ACKNOWLEDGEMENT_TIMED_OUT",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutboxFailureDisposition {
    pub retry_count: i32,
    pub dead_lettered: bool,
    pub available_at: DateTime<Utc>,
}

#[derive(Clone)]
pub struct PostgresOutboxLeaseRepository {
    pool: PgPool,
    worker_id: String,
    policy: OutboxLeasePolicy,
}

impl fmt::Debug for PostgresOutboxLeaseRepository {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresOutboxLeaseRepository")
            .field("pool", &"[POSTGRESQL POOL]")
            .field("worker_id", &self.worker_id)
            .field("policy", &self.policy)
            .finish()
    }
}

impl PostgresOutboxLeaseRepository {
    pub fn new(
        pool: PgPool,
        worker_id: impl Into<String>,
        policy: OutboxLeasePolicy,
    ) -> Result<Self, EventWorkerError> {
        let worker_id = worker_id.into();
        if !valid_worker_id(&worker_id) {
            return Err(EventWorkerError::Configuration("invalid_worker_id"));
        }
        Ok(Self {
            pool,
            worker_id,
            policy: policy.validate()?,
        })
    }

    pub fn worker_id(&self) -> &str {
        &self.worker_id
    }

    pub fn lease_duration(&self) -> Duration {
        self.policy.lease_duration
    }

    pub async fn claim_batch(&self, limit: i64) -> Result<Vec<OutboxClaim>, EventWorkerError> {
        if !(1..=1_000).contains(&limit) {
            return Err(EventWorkerError::Configuration(
                "outbox_claim_limit_out_of_range",
            ));
        }
        let lease_milliseconds = duration_milliseconds(self.policy.lease_duration).ok_or(
            EventWorkerError::Configuration("outbox_lease_duration_out_of_range"),
        )?;
        let claim_token_prefix = next_claim_token_prefix(&self.worker_id)?;
        sqlx::query_as::<_, OutboxClaim>(
            r#"
            WITH candidates AS (
                SELECT outbox_id
                  FROM public.event_outbox
                 WHERE published_at IS NULL
                   AND dead_lettered_at IS NULL
                   AND integrity_status = 'verified_hmac'
                   AND request_hash_source = 'formal_commit'
                   AND commit_id IS NOT NULL
                   AND EXISTS (
                       SELECT 1
                         FROM public.event_store AS event
                        WHERE event.sequence = event_outbox.event_sequence
                          AND (
                              event.data_subject_id = 'not_applicable'
                              OR EXISTS (
                                      SELECT 1
                                        FROM public.privacy_subject_keys AS subject_key
                                       WHERE subject_key.subject_id = event.data_subject_id
                                         AND subject_key.key_reference = event.payload_key_reference
                                         AND subject_key.wrapped_key IS NOT NULL
                                         AND subject_key.destroyed_at IS NULL
                                  )
                          )
                   )
                   AND available_at <= now()
                   AND (
                       delivery_status IN ('pending', 'retrying')
                       OR delivery_status = 'claimed' AND locked_until <= now()
                   )
                 ORDER BY available_at, outbox_id
                 FOR UPDATE SKIP LOCKED
                 LIMIT $2
            ), claimed AS (
                UPDATE public.event_outbox AS outbox
                   SET delivery_status = 'claimed',
                       claimed_at = now(),
                       claim_owner = $1,
                       locked_until = now() + ($3::bigint * interval '1 millisecond'),
                       claim_token = $4 || ':' || outbox.outbox_id::TEXT
                  FROM candidates
                 WHERE outbox.outbox_id = candidates.outbox_id
                RETURNING outbox.*
            )
            SELECT claimed.outbox_id, claimed.event_sequence,
                   claimed.nats_subject AS subject, claimed.idempotency_key,
                   claimed.visibility_label, claimed.correlation_id,
                   claimed.causation_id, claimed.payload_json,
                   claimed.commit_id, event.event_type,
                   event.event_schema_version, event.campaign_id,
                   event.stream_id, event.stream_version,
                   event.expected_version,
                   event.idempotency_key AS event_idempotency_key,
                   event.idempotency_operation,
                   event.authenticated_actor_id,
                   event.authenticated_actor_role,
                   event.authenticated_actor_origin,
                   event.resource_type, event.resource_id,
                   event.authority_contract_id, event.authority_owner,
                   event.authority_contract_version, event.command_id,
                   event.visibility_subject, event.data_subject_id,
                   event.fact_provenance_kind AS provenance_kind,
                   event.fact_provenance_reference AS provenance_reference,
                   event.fact_recorded_by AS provenance_recorded_by,
                   event.trace_id, event.recorded_at,
                   event.event_integrity_hash, event.request_hash,
                   event.request_hash_source, event.integrity_status,
                   event.payload_integrity_source, claimed.retry_count,
                   claimed.claimed_at, claimed.locked_until,
                   claimed.claim_token
              FROM claimed
              JOIN public.event_store AS event
                ON event.sequence = claimed.event_sequence
             ORDER BY claimed.outbox_id
            "#,
        )
        .bind(&self.worker_id)
        .bind(limit)
        .bind(lease_milliseconds)
        .bind(claim_token_prefix)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| EventWorkerError::Database("claim_outbox_batch"))
    }

    pub async fn mark_published(&self, claim: &OutboxClaim) -> Result<(), EventWorkerError> {
        let result = sqlx::query(
            r#"
            UPDATE public.event_outbox
               SET delivery_status = 'published',
                   published_at = now(),
                   available_at = now(),
                   claimed_at = NULL,
                   claim_owner = NULL,
                   claim_token = NULL,
                   locked_until = NULL,
                   last_error = NULL
             WHERE outbox_id = $1
               AND delivery_status = 'claimed'
               AND claim_owner = $2
               AND claim_token = $3
               AND claimed_at = $4
               AND locked_until = $5
               AND locked_until > now()
               AND published_at IS NULL
               AND dead_lettered_at IS NULL
            "#,
        )
        .bind(claim.outbox_id)
        .bind(&self.worker_id)
        .bind(&claim.claim_token)
        .bind(claim.claimed_at)
        .bind(claim.locked_until)
        .execute(&self.pool)
        .await
        .map_err(|_| EventWorkerError::Database("mark_outbox_published"))?;
        require_owned_claim(result.rows_affected())
    }

    /// Historical or otherwise unverified rows remain auditable in the
    /// outbox but are permanently quarantined before any NATS claim. They can
    /// never enter the shared canonical-events stream.
    pub async fn quarantine_unverified_history(&self) -> Result<u64, EventWorkerError> {
        let result = sqlx::query(
            r#"
            UPDATE public.event_outbox
               SET delivery_status = 'dead_lettered',
                   dead_lettered_at = COALESCE(dead_lettered_at, now()),
                   available_at = now(),
                   last_error = 'UNVERIFIED_HISTORY_QUARANTINED',
                   claimed_at = NULL,
                   claim_owner = NULL,
                   claim_token = NULL,
                   locked_until = NULL
             WHERE published_at IS NULL
               AND dead_lettered_at IS NULL
               AND (
                    integrity_status <> 'verified_hmac'
                    OR request_hash_source <> 'formal_commit'
                    OR commit_id IS NULL
                    OR EXISTS (
                        SELECT 1
                          FROM public.event_store AS event
                         WHERE event.sequence = event_outbox.event_sequence
                           AND event.data_subject_id <> 'not_applicable'
                           AND (
                               NOT EXISTS (
                                   SELECT 1
                                     FROM public.privacy_subject_keys AS subject_key
                                    WHERE subject_key.subject_id = event.data_subject_id
                                      AND subject_key.key_reference = event.payload_key_reference
                                      AND subject_key.wrapped_key IS NOT NULL
                                      AND subject_key.destroyed_at IS NULL
                               )
                           )
                    )
               )
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|_| EventWorkerError::Database("quarantine_unverified_outbox"))?;
        Ok(result.rows_affected())
    }

    pub async fn mark_failed(
        &self,
        claim: &OutboxClaim,
        failure: OutboxFailureCode,
    ) -> Result<OutboxFailureDisposition, EventWorkerError> {
        let next_attempt = claim.retry_count.saturating_add(1);
        let dead_lettered = next_attempt >= self.policy.maximum_attempts;
        let backoff_milliseconds =
            duration_milliseconds(self.policy.backoff_for_attempt(next_attempt)).ok_or(
                EventWorkerError::Configuration("outbox_backoff_out_of_range"),
            )?;
        let row = sqlx::query(
            r#"
            UPDATE public.event_outbox
               SET retry_count = retry_count + 1,
                   last_error = $6,
                   delivery_status = CASE
                       WHEN $7 THEN 'dead_lettered'
                       ELSE 'retrying'
                   END,
                   available_at = CASE
                       WHEN $7 THEN now()
                       ELSE now() + ($8::bigint * interval '1 millisecond')
                   END,
                   dead_lettered_at = CASE WHEN $7 THEN now() ELSE NULL END,
                   claimed_at = NULL,
                   claim_owner = NULL,
                   claim_token = NULL,
                   locked_until = NULL
             WHERE outbox_id = $1
               AND delivery_status = 'claimed'
               AND claim_owner = $2
               AND claim_token = $3
               AND claimed_at = $4
               AND locked_until = $5
               AND locked_until > now()
               AND published_at IS NULL
               AND dead_lettered_at IS NULL
            RETURNING retry_count, delivery_status, available_at
            "#,
        )
        .bind(claim.outbox_id)
        .bind(&self.worker_id)
        .bind(&claim.claim_token)
        .bind(claim.claimed_at)
        .bind(claim.locked_until)
        .bind(failure.as_str())
        .bind(dead_lettered)
        .bind(backoff_milliseconds)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| EventWorkerError::Database("mark_outbox_failed"))?
        .ok_or(EventWorkerError::ClaimLost)?;
        let delivery_status: String = row.get("delivery_status");
        Ok(OutboxFailureDisposition {
            retry_count: row.get("retry_count"),
            dead_lettered: delivery_status == "dead_lettered",
            available_at: row.get("available_at"),
        })
    }

    pub async fn pending_count(&self) -> Result<i64, EventWorkerError> {
        sqlx::query_scalar(
            "SELECT count(*) FROM public.event_outbox WHERE delivery_status IN ('pending', 'claimed', 'retrying')",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|_| EventWorkerError::Database("count_pending_outbox"))
    }

    /// A non-zero value is the persistent alert condition consumed by the
    /// service's metrics/alerting layer; dead letters are never auto-deleted.
    pub async fn dead_letter_count(&self) -> Result<i64, EventWorkerError> {
        sqlx::query_scalar(
            "SELECT count(*) FROM public.event_outbox WHERE delivery_status = 'dead_lettered'",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|_| EventWorkerError::Database("count_dead_letter_outbox"))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, FromRow)]
pub struct ProjectionCheckpointState {
    pub projection_name: String,
    pub campaign_id: String,
    pub stream_id: String,
    pub version: i64,
    pub last_event_sequence: i64,
    pub projection_hash: String,
    pub rebuilt_at: DateTime<Utc>,
}

impl ProjectionCheckpointState {
    fn genesis(projection_name: &str, campaign_id: &str, stream_id: &str) -> Self {
        Self {
            projection_name: projection_name.to_owned(),
            campaign_id: campaign_id.to_owned(),
            stream_id: stream_id.to_owned(),
            version: 0,
            last_event_sequence: 0,
            projection_hash: PROJECTION_HASH_GENESIS.to_owned(),
            rebuilt_at: Utc::now(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProjectionPage {
    start: ProjectionCheckpointState,
    target: ProjectionCheckpointState,
    events: Vec<CanonicalReplayEvent>,
}

impl ProjectionPage {
    pub fn start(&self) -> &ProjectionCheckpointState {
        &self.start
    }

    pub fn target(&self) -> &ProjectionCheckpointState {
        &self.target
    }

    pub fn events(&self) -> &[CanonicalReplayEvent] {
        &self.events
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CheckpointAdvance {
    Advanced(ProjectionCheckpointState),
    AlreadyApplied(ProjectionCheckpointState),
    NoEvents(ProjectionCheckpointState),
}

#[derive(Clone)]
pub struct PostgresProjectionWorker {
    pool: PgPool,
    projection_name: String,
    page_size: i64,
}

impl fmt::Debug for PostgresProjectionWorker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresProjectionWorker")
            .field("pool", &"[POSTGRESQL POOL]")
            .field("projection_name", &self.projection_name)
            .field("page_size", &self.page_size)
            .finish()
    }
}

impl PostgresProjectionWorker {
    pub fn new(
        pool: PgPool,
        projection_name: impl Into<String>,
        page_size: i64,
    ) -> Result<Self, EventWorkerError> {
        let projection_name = projection_name.into();
        if projection_name.trim().is_empty()
            || projection_name.len() > 128
            || !(1..=10_000).contains(&page_size)
        {
            return Err(EventWorkerError::Configuration(
                "invalid_projection_worker_configuration",
            ));
        }
        Ok(Self {
            pool,
            projection_name,
            page_size,
        })
    }

    pub async fn check_readiness(&self) -> Result<(), EventWorkerError> {
        let ready: bool = sqlx::query_scalar(
            "SELECT to_regclass('public.canonical_event_projection') IS NOT NULL AND to_regclass('public.projection_checkpoint') IS NOT NULL",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|_| EventWorkerError::Database("check_projection_readiness"))?;
        if ready {
            Ok(())
        } else {
            Err(EventWorkerError::Database("projection_schema_missing"))
        }
    }

    pub async fn checkpoint(
        &self,
        campaign_id: &str,
        stream_id: &str,
    ) -> Result<ProjectionCheckpointState, EventWorkerError> {
        validate_stream_scope(campaign_id, stream_id)?;
        let checkpoint = sqlx::query_as::<_, ProjectionCheckpointState>(
            r#"
            SELECT projection_name, campaign_id, stream_id, version,
                   last_event_sequence, projection_hash, rebuilt_at
              FROM public.projection_checkpoint
             WHERE projection_name = $1
               AND campaign_id = $2
               AND stream_id = $3
            "#,
        )
        .bind(&self.projection_name)
        .bind(campaign_id)
        .bind(stream_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| EventWorkerError::Database("load_projection_checkpoint"))?;
        Ok(checkpoint.unwrap_or_else(|| {
            ProjectionCheckpointState::genesis(&self.projection_name, campaign_id, stream_id)
        }))
    }

    /// Delete only this rebuildable projection stream and its cursor while
    /// holding the same transaction-scoped lock used by page application.
    /// Canonical Event Store rows are deliberately outside this operation.
    pub async fn reset_stream_to_genesis(
        &self,
        campaign_id: &str,
        stream_id: &str,
    ) -> Result<ProjectionCheckpointState, EventWorkerError> {
        validate_stream_scope(campaign_id, stream_id)?;
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| EventWorkerError::Database("begin_projection_reset"))?;
        self.lock_stream(&mut transaction, campaign_id, stream_id)
            .await?;
        self.delete_materialization(&mut transaction, campaign_id, stream_id)
            .await?;
        transaction
            .commit()
            .await
            .map_err(|_| EventWorkerError::Database("commit_projection_reset"))?;
        Ok(ProjectionCheckpointState::genesis(
            &self.projection_name,
            campaign_id,
            stream_id,
        ))
    }

    /// Force a complete reconstruction of one read-model stream from
    /// canonical Event Store history.
    pub async fn rebuild_from_genesis(
        &self,
        campaign_id: &str,
        stream_id: &str,
    ) -> Result<ProjectionCheckpointState, EventWorkerError> {
        self.reset_stream_to_genesis(campaign_id, stream_id).await?;
        self.rebuild_to_tip(campaign_id, stream_id).await
    }

    /// Compute a bounded projection page without mutating either the read
    /// model or its durable cursor. `advance_checkpoint` applies this page and
    /// advances the checkpoint in one PostgreSQL transaction.
    pub async fn prepare_page(
        &self,
        campaign_id: &str,
        stream_id: &str,
    ) -> Result<ProjectionPage, EventWorkerError> {
        let start = self.checkpoint(campaign_id, stream_id).await?;
        let rows = load_projection_events(
            &self.pool,
            campaign_id,
            stream_id,
            start.version,
            self.page_size,
        )
        .await?;
        let mut hasher = CanonicalProjectionHasher::resume(start.projection_hash.clone())?;
        let mut expected_version = start.version.saturating_add(1);
        for event in &rows {
            if event.stream_version != expected_version {
                return Err(EventWorkerError::ProjectionStreamGap {
                    expected: expected_version,
                    actual: event.stream_version,
                });
            }
            hasher.apply(event)?;
            expected_version = expected_version.saturating_add(1);
        }
        let target = if let Some(last) = rows.last() {
            ProjectionCheckpointState {
                projection_name: self.projection_name.clone(),
                campaign_id: campaign_id.to_owned(),
                stream_id: stream_id.to_owned(),
                version: last.stream_version,
                last_event_sequence: last.sequence,
                projection_hash: hasher.projection_hash().to_owned(),
                rebuilt_at: Utc::now(),
            }
        } else {
            start.clone()
        };
        Ok(ProjectionPage {
            start,
            target,
            events: rows,
        })
    }

    pub async fn advance_checkpoint(
        &self,
        page: &ProjectionPage,
    ) -> Result<CheckpointAdvance, EventWorkerError> {
        if page.events.is_empty() {
            return Ok(CheckpointAdvance::NoEvents(page.start.clone()));
        }
        let projection_hashes = validate_prepared_page(&self.projection_name, page)?;
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| EventWorkerError::Database("begin_checkpoint_transaction"))?;
        sqlx::query(
            "SELECT pg_advisory_xact_lock(hashtextextended($1 || ':' || $2 || ':' || $3, 0))",
        )
        .bind(&self.projection_name)
        .bind(&page.start.campaign_id)
        .bind(&page.start.stream_id)
        .execute(&mut *transaction)
        .await
        .map_err(|_| EventWorkerError::Database("lock_projection_checkpoint"))?;

        let current_row = sqlx::query_as::<_, ProjectionCheckpointState>(
            r#"
            SELECT projection_name, campaign_id, stream_id, version,
                   last_event_sequence, projection_hash, rebuilt_at
              FROM public.projection_checkpoint
             WHERE projection_name = $1
               AND campaign_id = $2
               AND stream_id = $3
             FOR UPDATE
            "#,
        )
        .bind(&self.projection_name)
        .bind(&page.start.campaign_id)
        .bind(&page.start.stream_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| EventWorkerError::Database("lock_current_checkpoint"))?;
        let checkpoint_exists = current_row.is_some();
        let current = current_row.unwrap_or_else(|| {
            ProjectionCheckpointState::genesis(
                &self.projection_name,
                &page.start.campaign_id,
                &page.start.stream_id,
            )
        });

        if current.version == page.target.version
            && current.last_event_sequence == page.target.last_event_sequence
            && current.projection_hash == page.target.projection_hash
        {
            let projected_rows: i64 = sqlx::query_scalar(
                r#"
                SELECT count(*)
                  FROM public.canonical_event_projection
                 WHERE projection_name = $1
                   AND campaign_id = $2
                   AND stream_id = $3
                   AND stream_version > $4
                   AND stream_version <= $5
                "#,
            )
            .bind(&self.projection_name)
            .bind(&page.start.campaign_id)
            .bind(&page.start.stream_id)
            .bind(page.start.version)
            .bind(page.target.version)
            .fetch_one(&mut *transaction)
            .await
            .map_err(|_| EventWorkerError::Database("verify_applied_projection_page"))?;
            if projected_rows != page.events.len() as i64 {
                return Err(EventWorkerError::ProjectionReadModelConflict);
            }
            transaction
                .commit()
                .await
                .map_err(|_| EventWorkerError::Database("commit_checkpoint_noop"))?;
            return Ok(CheckpointAdvance::AlreadyApplied(current));
        }
        if current.version != page.start.version
            || current.last_event_sequence != page.start.last_event_sequence
            || current.projection_hash != page.start.projection_hash
        {
            return Err(EventWorkerError::CheckpointConflict {
                expected_version: page.start.version,
                actual_version: current.version,
            });
        }

        for (event, projection_hash) in page.events.iter().zip(projection_hashes) {
            let event_document = serde_json::to_value(event)
                .map_err(|_| EventWorkerError::ProjectionSerialization)?;
            sqlx::query(
                r#"
                INSERT INTO public.canonical_event_projection (
                    projection_name, campaign_id, stream_id, stream_version,
                    event_sequence, projection_hash, event_document
                ) VALUES ($1, $2, $3, $4, $5, $6, $7)
                "#,
            )
            .bind(&self.projection_name)
            .bind(&event.campaign_id)
            .bind(&event.stream_id)
            .bind(event.stream_version)
            .bind(event.sequence)
            .bind(projection_hash)
            .bind(Json(event_document))
            .execute(&mut *transaction)
            .await
            .map_err(|_| EventWorkerError::Database("apply_projection_page"))?;
        }

        let advanced = if !checkpoint_exists {
            sqlx::query_as::<_, ProjectionCheckpointState>(
                r#"
                INSERT INTO public.projection_checkpoint (
                    projection_name, campaign_id, stream_id, version,
                    last_event_sequence, projection_hash, rebuilt_at
                ) VALUES ($1, $2, $3, $4, $5, $6, now())
                RETURNING projection_name, campaign_id, stream_id, version,
                          last_event_sequence, projection_hash, rebuilt_at
                "#,
            )
            .bind(&self.projection_name)
            .bind(&page.start.campaign_id)
            .bind(&page.start.stream_id)
            .bind(page.target.version)
            .bind(page.target.last_event_sequence)
            .bind(&page.target.projection_hash)
            .fetch_one(&mut *transaction)
            .await
            .map_err(|_| EventWorkerError::Database("insert_projection_checkpoint"))?
        } else {
            sqlx::query_as::<_, ProjectionCheckpointState>(
                r#"
                UPDATE public.projection_checkpoint
                   SET version = $4,
                       last_event_sequence = $5,
                       projection_hash = $6,
                       rebuilt_at = now()
                 WHERE projection_name = $1
                   AND campaign_id = $2
                   AND stream_id = $3
                   AND version = $7
                   AND last_event_sequence = $8
                   AND projection_hash = $9
                RETURNING projection_name, campaign_id, stream_id, version,
                          last_event_sequence, projection_hash, rebuilt_at
                "#,
            )
            .bind(&self.projection_name)
            .bind(&page.start.campaign_id)
            .bind(&page.start.stream_id)
            .bind(page.target.version)
            .bind(page.target.last_event_sequence)
            .bind(&page.target.projection_hash)
            .bind(page.start.version)
            .bind(page.start.last_event_sequence)
            .bind(&page.start.projection_hash)
            .fetch_one(&mut *transaction)
            .await
            .map_err(|_| EventWorkerError::Database("update_projection_checkpoint"))?
        };
        transaction
            .commit()
            .await
            .map_err(|_| EventWorkerError::Database("commit_projection_checkpoint"))?;
        Ok(CheckpointAdvance::Advanced(advanced))
    }

    pub async fn run_page(
        &self,
        campaign_id: &str,
        stream_id: &str,
    ) -> Result<CheckpointAdvance, EventWorkerError> {
        let page = self.prepare_page(campaign_id, stream_id).await?;
        self.advance_checkpoint(&page).await
    }

    pub async fn rebuild_to_tip(
        &self,
        campaign_id: &str,
        stream_id: &str,
    ) -> Result<ProjectionCheckpointState, EventWorkerError> {
        self.repair_materialization_if_inconsistent(campaign_id, stream_id)
            .await?;
        loop {
            match self.run_page(campaign_id, stream_id).await? {
                CheckpointAdvance::Advanced(_) | CheckpointAdvance::AlreadyApplied(_) => {}
                CheckpointAdvance::NoEvents(checkpoint) => {
                    // The empty-page path must not become a false pass when a
                    // read model was deleted while its checkpoint survived.
                    if self
                        .repair_materialization_if_inconsistent(campaign_id, stream_id)
                        .await?
                    {
                        continue;
                    }
                    return Ok(checkpoint);
                }
            }
        }
    }

    pub async fn rebuild_all_to_tip(
        &self,
    ) -> Result<Vec<ProjectionCheckpointState>, EventWorkerError> {
        let streams = sqlx::query_as::<_, ProjectionStreamScope>(
            r#"
            SELECT DISTINCT campaign_id, stream_id
              FROM public.event_store
             ORDER BY campaign_id, stream_id
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|_| EventWorkerError::Database("load_projection_streams"))?;
        let mut checkpoints = Vec::with_capacity(streams.len());
        for stream in streams {
            checkpoints.push(
                self.rebuild_to_tip(&stream.campaign_id, &stream.stream_id)
                    .await?,
            );
        }
        Ok(checkpoints)
    }

    async fn repair_materialization_if_inconsistent(
        &self,
        campaign_id: &str,
        stream_id: &str,
    ) -> Result<bool, EventWorkerError> {
        validate_stream_scope(campaign_id, stream_id)?;
        let mut transaction = self
            .pool
            .begin()
            .await
            .map_err(|_| EventWorkerError::Database("begin_projection_consistency_check"))?;
        self.lock_stream(&mut transaction, campaign_id, stream_id)
            .await?;

        let checkpoint = sqlx::query_as::<_, ProjectionCheckpointState>(
            r#"
            SELECT projection_name, campaign_id, stream_id, version,
                   last_event_sequence, projection_hash, rebuilt_at
              FROM public.projection_checkpoint
             WHERE projection_name = $1
               AND campaign_id = $2
               AND stream_id = $3
             FOR UPDATE
            "#,
        )
        .bind(&self.projection_name)
        .bind(campaign_id)
        .bind(stream_id)
        .fetch_optional(&mut *transaction)
        .await
        .map_err(|_| EventWorkerError::Database("inspect_projection_checkpoint"))?;
        let summary = sqlx::query_as::<_, ProjectionMaterializationSummary>(
            r#"
            SELECT count(*) AS projected_rows,
                   min(stream_version) AS first_version,
                   max(stream_version) AS last_version,
                   (array_agg(event_sequence ORDER BY stream_version DESC))[1]
                       AS last_event_sequence,
                   (array_agg(projection_hash ORDER BY stream_version DESC))[1]
                       AS last_projection_hash
              FROM public.canonical_event_projection
             WHERE projection_name = $1
               AND campaign_id = $2
               AND stream_id = $3
            "#,
        )
        .bind(&self.projection_name)
        .bind(campaign_id)
        .bind(stream_id)
        .fetch_one(&mut *transaction)
        .await
        .map_err(|_| EventWorkerError::Database("inspect_projection_materialization"))?;

        if summary.matches(checkpoint.as_ref()) {
            transaction
                .commit()
                .await
                .map_err(|_| EventWorkerError::Database("commit_projection_consistency_check"))?;
            return Ok(false);
        }

        self.delete_materialization(&mut transaction, campaign_id, stream_id)
            .await?;
        transaction
            .commit()
            .await
            .map_err(|_| EventWorkerError::Database("commit_projection_repair"))?;
        Ok(true)
    }

    async fn lock_stream(
        &self,
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        campaign_id: &str,
        stream_id: &str,
    ) -> Result<(), EventWorkerError> {
        sqlx::query(
            "SELECT pg_advisory_xact_lock(hashtextextended($1 || ':' || $2 || ':' || $3, 0))",
        )
        .bind(&self.projection_name)
        .bind(campaign_id)
        .bind(stream_id)
        .execute(&mut **transaction)
        .await
        .map_err(|_| EventWorkerError::Database("lock_projection_repair"))?;
        Ok(())
    }

    async fn delete_materialization(
        &self,
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        campaign_id: &str,
        stream_id: &str,
    ) -> Result<(), EventWorkerError> {
        sqlx::query(
            r#"
            DELETE FROM public.canonical_event_projection
             WHERE projection_name = $1
               AND campaign_id = $2
               AND stream_id = $3
            "#,
        )
        .bind(&self.projection_name)
        .bind(campaign_id)
        .bind(stream_id)
        .execute(&mut **transaction)
        .await
        .map_err(|_| EventWorkerError::Database("delete_projection_materialization"))?;
        sqlx::query(
            r#"
            DELETE FROM public.projection_checkpoint
             WHERE projection_name = $1
               AND campaign_id = $2
               AND stream_id = $3
            "#,
        )
        .bind(&self.projection_name)
        .bind(campaign_id)
        .bind(stream_id)
        .execute(&mut **transaction)
        .await
        .map_err(|_| EventWorkerError::Database("delete_projection_checkpoint"))?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EventWorkerError {
    Configuration(&'static str),
    Database(&'static str),
    ClaimLost,
    InvalidOutboxPayload,
    ProjectionHash(ProjectionHashError),
    ProjectionSerialization,
    ProjectionReadModelConflict,
    ProjectionStreamGap {
        expected: i64,
        actual: i64,
    },
    CheckpointIdentityMismatch,
    CheckpointConflict {
        expected_version: i64,
        actual_version: i64,
    },
}

impl fmt::Display for EventWorkerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration(reason) => write!(formatter, "worker configuration error: {reason}"),
            Self::Database(operation) => write!(formatter, "event worker database failed: {operation}"),
            Self::ClaimLost => formatter.write_str("outbox claim is no longer owned by this worker"),
            Self::InvalidOutboxPayload => formatter.write_str("invalid canonical outbox payload"),
            Self::ProjectionHash(error) => error.fmt(formatter),
            Self::ProjectionSerialization => {
                formatter.write_str("projection event serialization failed")
            }
            Self::ProjectionReadModelConflict => {
                formatter.write_str("projection read model does not match its checkpoint")
            }
            Self::ProjectionStreamGap { expected, actual } => write!(
                formatter,
                "projection stream gap: expected version {expected}, actual version {actual}"
            ),
            Self::CheckpointIdentityMismatch => {
                formatter.write_str("projection page identity does not match worker")
            }
            Self::CheckpointConflict {
                expected_version,
                actual_version,
            } => write!(
                formatter,
                "checkpoint conflict: expected version {expected_version}, actual version {actual_version}"
            ),
        }
    }
}

impl std::error::Error for EventWorkerError {}

impl From<ProjectionHashError> for EventWorkerError {
    fn from(error: ProjectionHashError) -> Self {
        Self::ProjectionHash(error)
    }
}

#[derive(FromRow)]
struct ProjectionStreamScope {
    campaign_id: String,
    stream_id: String,
}

#[derive(FromRow)]
struct ProjectionMaterializationSummary {
    projected_rows: i64,
    first_version: Option<i64>,
    last_version: Option<i64>,
    last_event_sequence: Option<i64>,
    last_projection_hash: Option<String>,
}

impl ProjectionMaterializationSummary {
    fn matches(&self, checkpoint: Option<&ProjectionCheckpointState>) -> bool {
        match checkpoint {
            None => {
                self.projected_rows == 0
                    && self.first_version.is_none()
                    && self.last_version.is_none()
                    && self.last_event_sequence.is_none()
                    && self.last_projection_hash.is_none()
            }
            Some(checkpoint) if checkpoint.version == 0 => {
                checkpoint.last_event_sequence == 0
                    && checkpoint.projection_hash == PROJECTION_HASH_GENESIS
                    && self.projected_rows == 0
                    && self.first_version.is_none()
                    && self.last_version.is_none()
                    && self.last_event_sequence.is_none()
                    && self.last_projection_hash.is_none()
            }
            Some(checkpoint) => {
                self.projected_rows == checkpoint.version
                    && self.first_version == Some(1)
                    && self.last_version == Some(checkpoint.version)
                    && self.last_event_sequence == Some(checkpoint.last_event_sequence)
                    && self.last_projection_hash.as_deref()
                        == Some(checkpoint.projection_hash.as_str())
            }
        }
    }
}

#[derive(FromRow)]
struct ProjectionEventRow {
    sequence: i64,
    stream_version: i64,
    stream_id: String,
    event_type: String,
    event_schema_version: i32,
    campaign_id: String,
    expected_version: i64,
    authority_mode: String,
    authenticated_actor_id: String,
    authenticated_actor_role: String,
    authenticated_actor_origin: Json<EventActorOriginWire>,
    resource_type: String,
    resource_id: String,
    authority_contract_id: String,
    authority_owner: String,
    command_id: String,
    idempotency_key: String,
    idempotency_operation: String,
    authority_contract_version: i64,
    visibility_label: String,
    visibility_subject: String,
    provenance_kind: String,
    provenance_reference: String,
    provenance_recorded_by: String,
    correlation_id: String,
    causation_id: String,
    trace_id: String,
    payload: Value,
    recorded_at: DateTime<Utc>,
    event_integrity_hash: Option<String>,
    request_hash: String,
    request_hash_source: String,
    integrity_status: String,
    payload_integrity_source: String,
}

impl From<ProjectionEventRow> for CanonicalReplayEvent {
    fn from(row: ProjectionEventRow) -> Self {
        Self {
            sequence: row.sequence,
            stream_version: row.stream_version,
            stream_id: row.stream_id,
            event_type: row.event_type,
            event_schema_version: row.event_schema_version,
            campaign_id: row.campaign_id,
            expected_version: row.expected_version,
            authority_mode: row.authority_mode,
            authenticated_actor_id: row.authenticated_actor_id,
            authenticated_actor_role: row.authenticated_actor_role,
            authenticated_actor_origin: row.authenticated_actor_origin.0,
            resource_type: row.resource_type,
            resource_id: row.resource_id,
            authority_contract_id: row.authority_contract_id,
            authority_owner: row.authority_owner,
            command_id: row.command_id,
            idempotency_key: row.idempotency_key,
            idempotency_operation: row.idempotency_operation,
            authority_contract_version: row.authority_contract_version,
            visibility_label: row.visibility_label,
            visibility_subject: row.visibility_subject,
            provenance_kind: row.provenance_kind,
            provenance_reference: row.provenance_reference,
            provenance_recorded_by: row.provenance_recorded_by,
            correlation_id: row.correlation_id,
            causation_id: row.causation_id,
            trace_id: row.trace_id,
            payload: row.payload,
            recorded_at: row.recorded_at,
            event_integrity_hash: row.event_integrity_hash,
            request_hash: row.request_hash,
            request_hash_source: row.request_hash_source,
            integrity_status: row.integrity_status,
            payload_integrity_source: row.payload_integrity_source,
        }
    }
}

async fn load_projection_events(
    pool: &PgPool,
    campaign_id: &str,
    stream_id: &str,
    after_version: i64,
    limit: i64,
) -> Result<Vec<CanonicalReplayEvent>, EventWorkerError> {
    let rows = sqlx::query_as::<_, ProjectionEventRow>(
        r#"
        SELECT sequence, stream_version, stream_id, event_type,
               event_schema_version, campaign_id, expected_version,
               authority_mode,
               authenticated_actor_id, authenticated_actor_role,
               authenticated_actor_origin, resource_type, resource_id,
               authority_contract_id, authority_owner, command_id,
               idempotency_key, idempotency_operation,
               authority_contract_version, visibility_label,
               visibility_subject, fact_provenance_kind AS provenance_kind,
               fact_provenance_reference AS provenance_reference,
               fact_recorded_by AS provenance_recorded_by, correlation_id,
               causation_id, trace_id, payload_json AS payload,
               recorded_at, event_integrity_hash, request_hash,
               request_hash_source, integrity_status,
               payload_integrity_source
          FROM public.event_store
         WHERE campaign_id = $1
           AND stream_id = $2
           AND stream_version > $3
           AND integrity_status = 'verified_hmac'
           AND request_hash_source = 'formal_commit'
           AND event_integrity_hash IS NOT NULL
           AND payload_json ? 'protected_payload'
         ORDER BY stream_version
         LIMIT $4
        "#,
    )
    .bind(campaign_id)
    .bind(stream_id)
    .bind(after_version)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|_| EventWorkerError::Database("load_projection_page"))?;
    Ok(rows.into_iter().map(Into::into).collect())
}

fn validate_prepared_page(
    projection_name: &str,
    page: &ProjectionPage,
) -> Result<Vec<String>, EventWorkerError> {
    if page.start.projection_name != projection_name
        || page.target.projection_name != projection_name
        || page.start.campaign_id != page.target.campaign_id
        || page.start.stream_id != page.target.stream_id
        || page.target.version < page.start.version
        || page.target.last_event_sequence < page.start.last_event_sequence
    {
        return Err(EventWorkerError::CheckpointIdentityMismatch);
    }
    let mut hasher = CanonicalProjectionHasher::resume(page.start.projection_hash.clone())?;
    let mut expected_version = page.start.version.saturating_add(1);
    let mut previous_sequence = page.start.last_event_sequence;
    let mut hashes = Vec::with_capacity(page.events.len());
    for event in &page.events {
        if event.campaign_id != page.start.campaign_id
            || event.stream_id != page.start.stream_id
            || event.sequence <= previous_sequence
        {
            return Err(EventWorkerError::CheckpointIdentityMismatch);
        }
        if event.stream_version != expected_version {
            return Err(EventWorkerError::ProjectionStreamGap {
                expected: expected_version,
                actual: event.stream_version,
            });
        }
        hasher.apply(event)?;
        hashes.push(hasher.projection_hash().to_owned());
        previous_sequence = event.sequence;
        expected_version = expected_version.saturating_add(1);
    }
    let Some(last) = page.events.last() else {
        return Err(EventWorkerError::CheckpointIdentityMismatch);
    };
    if page.target.version != last.stream_version
        || page.target.last_event_sequence != last.sequence
        || page.target.projection_hash != hasher.projection_hash()
    {
        return Err(EventWorkerError::CheckpointIdentityMismatch);
    }
    Ok(hashes)
}

fn validate_stream_scope(campaign_id: &str, stream_id: &str) -> Result<(), EventWorkerError> {
    if campaign_id.trim().is_empty()
        || stream_id.trim().is_empty()
        || campaign_id.len() > 256
        || stream_id.len() > 256
    {
        Err(EventWorkerError::Configuration("invalid_stream_scope"))
    } else {
        Ok(())
    }
}

fn valid_worker_id(worker_id: &str) -> bool {
    !worker_id.trim().is_empty()
        && worker_id.len() <= 128
        && worker_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

static CLAIM_TOKEN_SEQUENCE: AtomicU64 = AtomicU64::new(1);

fn next_claim_token_prefix(worker_id: &str) -> Result<String, EventWorkerError> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| EventWorkerError::Configuration("system_clock_before_unix_epoch"))?
        .as_nanos();
    let sequence = CLAIM_TOKEN_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let mut digest = Sha256::new();
    digest.update(b"trpg-outbox-claim-token-v1");
    digest.update((worker_id.len() as u64).to_be_bytes());
    digest.update(worker_id.as_bytes());
    digest.update(timestamp.to_be_bytes());
    digest.update(std::process::id().to_be_bytes());
    digest.update(sequence.to_be_bytes());
    Ok(format!("claim-sha256:{:x}", digest.finalize()))
}

fn duration_milliseconds(duration: Duration) -> Option<i64> {
    i64::try_from(duration.as_millis()).ok()
}

fn require_owned_claim(rows_affected: u64) -> Result<(), EventWorkerError> {
    if rows_affected == 1 {
        Ok(())
    } else {
        Err(EventWorkerError::ClaimLost)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exponential_backoff_is_bounded() {
        let policy = OutboxLeasePolicy {
            lease_duration: Duration::from_secs(30),
            initial_backoff: Duration::from_secs(2),
            maximum_backoff: Duration::from_secs(10),
            maximum_attempts: 5,
        };
        assert_eq!(policy.backoff_for_attempt(1), Duration::from_secs(2));
        assert_eq!(policy.backoff_for_attempt(2), Duration::from_secs(4));
        assert_eq!(policy.backoff_for_attempt(3), Duration::from_secs(8));
        assert_eq!(policy.backoff_for_attempt(4), Duration::from_secs(10));
        assert_eq!(policy.backoff_for_attempt(100), Duration::from_secs(10));
    }

    #[test]
    fn worker_identifiers_cannot_inject_transport_metadata() {
        assert!(valid_worker_id("outbox-worker_01"));
        assert!(!valid_worker_id("outbox worker"));
        assert!(!valid_worker_id("outbox\nworker"));
    }

    #[test]
    fn claim_tokens_are_unique_and_worker_scoped() {
        let first = next_claim_token_prefix("outbox-worker_01").unwrap();
        let second = next_claim_token_prefix("outbox-worker_01").unwrap();
        let peer = next_claim_token_prefix("outbox-worker_02").unwrap();
        assert_ne!(first, second);
        assert_ne!(second, peer);
        assert!(first.starts_with("claim-sha256:"));
    }
}
