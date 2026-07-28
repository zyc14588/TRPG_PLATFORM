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
