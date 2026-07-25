crate::define_data_event_module!(
    EventBusNatsImplCommand,
    EventBusNatsImplOperation,
    append_event_bus_nats_impl_event,
    "event_bus_nats_impl",
    "EventBusNatsImplRecorded",
    "data_eventing.event_bus_nats_impl.event_schema",
    crate::DataEventOperation::OutboxPublish,
    [
        "event_outbox",
        "nats_jetstream_consumer",
        "dead_letter_queue"
    ]
);

crate::define_data_event_artifacts!(
    EventBusNatsImplService,
    EventBusNatsImplRepository,
    EventBusNatsImplEvent,
    EventBusNatsImplError,
    EVENT_TYPE,
    EVENT_SCHEMA_NAME
);

pub const OUTBOX_FLOW_STATES: &[&str] = &[
    "pending",
    "claimed",
    "published",
    "retrying",
    "dead_lettered",
];
pub const PUBLISH_SOURCE: &str = crate::OUTBOX_TABLE;

use std::fmt;
use std::path::Path;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

#[cfg(test)]
use crate::event_store_sqlx_outbox_projection::PayloadCipher;
use async_nats::jetstream::stream::{
    Config as StreamConfig, DiscardPolicy, RetentionPolicy, StorageType,
};
use async_nats::{ConnectOptions, HeaderMap, HeaderValue};
use percent_encoding::percent_decode_str;
use sha2::{Digest, Sha256};
#[cfg(test)]
use trpg_shared_kernel::EventActorOriginWire;
use trpg_shared_kernel::{EventEnvelopeWire, EVENT_ENVELOPE_WIRE_SCHEMA_VERSION};
use url::Url;

use crate::event_store_sqlx_outbox_projection::PostgresCanonicalStore;
use crate::outbox_projection_workers::{
    EventWorkerError, EventingMetrics, OutboxClaim, OutboxFailureCode, OutboxLeasePolicy,
    PostgresOutboxLeaseRepository, PostgresProjectionWorker, ProjectionCheckpointState,
};
use crate::postgre_sql_sq_lx_pgvector::PostgresRagSnapshotRepository;

const STREAM_NAME: &str = "TRPG_CANONICAL_EVENTS";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PublishBatchResult {
    pub claimed: usize,
    pub published: usize,
    pub failed: usize,
    pub dead_lettered: usize,
    /// Persistent total, including rows dead-lettered by earlier process
    /// instances. A non-zero value must keep readiness degraded until an
    /// operator explicitly remediates the durable rows.
    pub dead_letter_total: i64,
}

impl PublishBatchResult {
    pub const fn requires_operator_attention(self) -> bool {
        self.dead_lettered > 0 || self.dead_letter_total > 0
    }

    pub const fn alert_code(self) -> Option<&'static str> {
        if self.requires_operator_attention() {
            Some("OUTBOX_DEAD_LETTER_ALERT")
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JetStreamOutboxError {
    Configuration(&'static str),
    PostgresUnavailable,
    NatsUnavailable,
    StreamUnavailable,
    PublishAcknowledgementTimedOut,
    Database(&'static str),
    InvalidOutboxPayload,
}

impl fmt::Display for JetStreamOutboxError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration(reason) => {
                write!(formatter, "outbox configuration error: {reason}")
            }
            Self::PostgresUnavailable => formatter.write_str("outbox PostgreSQL unavailable"),
            Self::NatsUnavailable => formatter.write_str("NATS unavailable"),
            Self::StreamUnavailable => formatter.write_str("JetStream stream unavailable"),
            Self::PublishAcknowledgementTimedOut => {
                formatter.write_str("JetStream publish acknowledgement timed out")
            }
            Self::Database(operation) => write!(formatter, "outbox database failed: {operation}"),
            Self::InvalidOutboxPayload => formatter.write_str("invalid outbox payload"),
        }
    }
}

impl std::error::Error for JetStreamOutboxError {}

#[derive(Clone)]
pub struct JetStreamOutboxPublisher {
    canonical: PostgresCanonicalStore,
    repository: PostgresOutboxLeaseRepository,
    projection: PostgresProjectionWorker,
    rag: PostgresRagSnapshotRepository,
    jetstream: async_nats::jetstream::Context,
    metrics: Arc<EventingMetrics>,
    batch_size: i64,
}

impl fmt::Debug for JetStreamOutboxPublisher {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JetStreamOutboxPublisher")
            .field("canonical", &self.canonical)
            .field("repository", &self.repository)
            .field("projection", &self.projection)
            .field("rag", &self.rag)
            .field("jetstream", &"[JETSTREAM CONTEXT]")
            .field("metrics", &self.metrics)
            .field("batch_size", &self.batch_size)
            .finish()
    }
}

impl JetStreamOutboxPublisher {
    pub async fn connect(
        canonical: PostgresCanonicalStore,
        nats_url: &str,
        worker_id: &str,
        nats_ca_certificate_path: Option<&Path>,
    ) -> Result<Self, JetStreamOutboxError> {
        Self::connect_with_credentials(
            canonical,
            nats_url,
            worker_id,
            nats_ca_certificate_path,
            None,
            None,
            None,
        )
        .await
    }

    pub async fn connect_with_credentials(
        canonical: PostgresCanonicalStore,
        nats_url: &str,
        worker_id: &str,
        nats_ca_certificate_path: Option<&Path>,
        nats_client_certificate_path: Option<&Path>,
        nats_client_private_key_path: Option<&Path>,
        nats_credentials_path: Option<&Path>,
    ) -> Result<Self, JetStreamOutboxError> {
        validate_worker_id(worker_id)?;
        canonical
            .verify_integrity()
            .await
            .map_err(|_| JetStreamOutboxError::Database("canonical_integrity_verification"))?;
        let pool = canonical.primary_pool();

        let (local_nats, tls_nats) = validate_nats_url(nats_url)?;
        let url_credentials = nats_url_credentials(nats_url)?;
        let connection_url = nats_endpoint_without_userinfo(nats_url)?;
        if !local_nats && nats_credentials_path.is_none() && url_credentials.is_none() {
            return Err(JetStreamOutboxError::Configuration(
                "remote_nats_credentials_required",
            ));
        }
        if nats_credentials_path.is_some() && url_credentials.is_some() {
            return Err(JetStreamOutboxError::Configuration(
                "ambiguous_nats_credentials",
            ));
        }
        if nats_client_certificate_path.is_some() != nats_client_private_key_path.is_some() {
            return Err(JetStreamOutboxError::Configuration(
                "nats_client_certificate_and_key_required_together",
            ));
        }
        let mut options = ConnectOptions::new()
            .name(worker_id)
            .require_tls(tls_nats || !local_nats)
            .connection_timeout(Duration::from_secs(5));
        if let Some(path) = nats_ca_certificate_path {
            options = options.add_root_certificates(path.to_path_buf());
        }
        if let (Some(certificate), Some(private_key)) =
            (nats_client_certificate_path, nats_client_private_key_path)
        {
            options = options
                .add_client_certificate(certificate.to_path_buf(), private_key.to_path_buf());
        }
        if let Some((username, password)) = url_credentials {
            options = options.user_and_password(username, password);
        } else if let Some(path) = nats_credentials_path {
            options = options
                .credentials_file(path)
                .await
                .map_err(|_| JetStreamOutboxError::Configuration("invalid_nats_credentials"))?;
        }
        let client = options
            .connect(connection_url)
            .await
            .map_err(|_| JetStreamOutboxError::NatsUnavailable)?;
        let repository = PostgresOutboxLeaseRepository::new(
            pool.clone(),
            worker_id,
            OutboxLeasePolicy::default(),
        )
        .map_err(map_worker_error)?;
        let projection =
            PostgresProjectionWorker::new(pool.clone(), "canonical_event_projection", 250)
                .map_err(map_worker_error)?;
        let rag = PostgresRagSnapshotRepository::new(pool);
        Ok(Self {
            canonical,
            repository,
            projection,
            rag,
            jetstream: async_nats::jetstream::new(client),
            metrics: Arc::new(EventingMetrics::default()),
            batch_size: 100,
        })
    }

    pub fn with_metrics(mut self, metrics: Arc<EventingMetrics>) -> Self {
        self.metrics = metrics;
        self
    }

    pub fn metrics(&self) -> Arc<EventingMetrics> {
        Arc::clone(&self.metrics)
    }

    pub async fn ensure_stream(&self) -> Result<(), JetStreamOutboxError> {
        let desired = canonical_stream_config();
        let mut stream = self
            .jetstream
            .get_or_create_stream(desired.clone())
            .await
            .map_err(|_| JetStreamOutboxError::StreamUnavailable)?;
        let info = stream
            .info()
            .await
            .map_err(|_| JetStreamOutboxError::StreamUnavailable)?;
        if !stream_config_matches(&info.config, &desired) {
            return Err(JetStreamOutboxError::Configuration(
                "jetstream_stream_contract_mismatch",
            ));
        }
        Ok(())
    }

    pub async fn check_readiness(&self) -> Result<(), JetStreamOutboxError> {
        self.canonical
            .verify_integrity()
            .await
            .map_err(|_| JetStreamOutboxError::Database("canonical_integrity_verification"))?;
        self.ensure_stream().await?;
        self.projection
            .check_readiness()
            .await
            .map_err(map_worker_error)?;
        self.rag
            .check_readiness()
            .await
            .map_err(|_| JetStreamOutboxError::Database("rag_read_model_readiness"))
    }

    pub async fn rebuild_projections_to_tip(
        &self,
    ) -> Result<Vec<ProjectionCheckpointState>, JetStreamOutboxError> {
        self.canonical
            .verify_integrity()
            .await
            .map_err(|_| JetStreamOutboxError::Database("canonical_integrity_verification"))?;
        self.projection
            .rebuild_all_to_tip()
            .await
            .map_err(map_worker_error)
    }

    pub async fn publish_batch(&self) -> Result<PublishBatchResult, JetStreamOutboxError> {
        self.canonical
            .verify_integrity()
            .await
            .map_err(|_| JetStreamOutboxError::Database("canonical_integrity_verification"))?;
        let mut result = PublishBatchResult::default();
        self.repository
            .quarantine_unverified_history()
            .await
            .map_err(map_worker_error)?;
        let acknowledgement_budget = self
            .repository
            .lease_duration()
            .checked_div(2)
            .filter(|duration| !duration.is_zero())
            .ok_or(JetStreamOutboxError::Configuration(
                "outbox_lease_too_short_for_publish",
            ))?;
        // Claim immediately before each external publish. This keeps rows that
        // are later in the configured batch out of a ticking lease while an
        // earlier JetStream acknowledgement is pending.
        for _ in 0..self.batch_size {
            let mut claimed = self
                .repository
                .claim_batch(1)
                .await
                .map_err(map_worker_error)?;
            let Some(row) = claimed.pop() else {
                break;
            };
            self.canonical
                .verify_integrity()
                .await
                .map_err(|_| JetStreamOutboxError::Database("canonical_integrity_verification"))?;
            result.claimed += 1;
            let publish_result =
                tokio::time::timeout(acknowledgement_budget, self.publish_one(&row))
                    .await
                    .map_err(|_| JetStreamOutboxError::PublishAcknowledgementTimedOut)
                    .and_then(|result| result);
            match publish_result {
                Ok(()) => {
                    self.repository
                        .mark_published(&row)
                        .await
                        .map_err(map_worker_error)?;
                    self.metrics.record_outbox_publish(&row, "published");
                    result.published += 1;
                }
                Err(error) => {
                    let failure = match error {
                        JetStreamOutboxError::InvalidOutboxPayload => {
                            OutboxFailureCode::InvalidEnvelope
                        }
                        JetStreamOutboxError::PublishAcknowledgementTimedOut => {
                            OutboxFailureCode::PublishAcknowledgementTimedOut
                        }
                        _ => OutboxFailureCode::JetStreamPublishFailed,
                    };
                    let disposition = self
                        .repository
                        .mark_failed(&row, failure)
                        .await
                        .map_err(map_worker_error)?;
                    self.metrics.record_outbox_publish(&row, "failed");
                    result.failed += 1;
                    if disposition.dead_lettered {
                        result.dead_lettered += 1;
                    }
                }
            }
        }
        result.dead_letter_total = self
            .repository
            .dead_letter_count()
            .await
            .map_err(map_worker_error)?;
        Ok(result)
    }

    pub async fn pending_count(&self) -> Result<i64, JetStreamOutboxError> {
        self.repository
            .pending_count()
            .await
            .map_err(map_worker_error)
    }

    pub async fn stream_message_count(&self) -> Result<u64, JetStreamOutboxError> {
        let mut stream = self
            .jetstream
            .get_stream(STREAM_NAME)
            .await
            .map_err(|_| JetStreamOutboxError::StreamUnavailable)?;
        Ok(stream
            .info()
            .await
            .map_err(|_| JetStreamOutboxError::StreamUnavailable)?
            .state
            .messages)
    }

    #[tracing::instrument(
        name = "jetstream_outbox_publish",
        skip_all,
        fields(
            correlation_id = %row.correlation_id,
            causation_id = %row.causation_id,
            event_sequence = row.event_sequence,
            campaign_id = %row.campaign_id,
            visibility_label = %row.visibility_label,
            provenance_kind = %row.provenance_kind
        )
    )]
    async fn publish_one(&self, row: &OutboxClaim) -> Result<(), JetStreamOutboxError> {
        // Validate after claiming so one corrupt row follows the ordinary
        // per-row failure/dead-letter path without retaining every other
        // claim in the batch until the lease expires.
        row.validate_for_publish().map_err(map_worker_error)?;
        let envelope = serde_json::to_vec(&event_envelope(row)?)
            .map_err(|_| JetStreamOutboxError::InvalidOutboxPayload)?;
        let headers = outbox_headers(row)?;
        self.jetstream
            .publish_with_headers(canonical_delivery_subject(row), headers, envelope.into())
            .await
            .map_err(|_| JetStreamOutboxError::NatsUnavailable)?
            .await
            .map_err(|_| JetStreamOutboxError::NatsUnavailable)?;
        Ok(())
    }
}

fn canonical_stream_config() -> StreamConfig {
    StreamConfig {
        name: STREAM_NAME.to_owned(),
        description: Some("Canonical TRPG event outbox".to_owned()),
        subjects: vec!["trpg.events.>".to_owned()],
        retention: RetentionPolicy::Limits,
        discard: DiscardPolicy::Old,
        max_bytes: 10 * 1024 * 1024 * 1024,
        max_messages: -1,
        max_messages_per_subject: -1,
        max_consumers: -1,
        max_age: Duration::from_secs(7 * 24 * 60 * 60),
        max_message_size: -1,
        duplicate_window: Duration::from_secs(120),
        storage: StorageType::File,
        num_replicas: 1,
        no_ack: false,
        // Exact data-subject messages may be removed by the privacy worker.
        // Whole-stream purge remains prohibited.
        deny_delete: false,
        deny_purge: true,
        // NATS 2.10 normalizes an omitted compression override to an explicit
        // `none`; make the canonical contract explicit so fail-closed
        // comparison does not mistake server normalization for drift.
        compression: Some(async_nats::jetstream::stream::Compression::None),
        ..Default::default()
    }
}

fn stream_config_matches(actual: &StreamConfig, desired: &StreamConfig) -> bool {
    let mut actual_subjects = actual.subjects.clone();
    let mut desired_subjects = desired.subjects.clone();
    actual_subjects.sort_unstable();
    desired_subjects.sort_unstable();
    actual.name == desired.name
        && actual.description == desired.description
        && actual_subjects == desired_subjects
        && actual.max_bytes == desired.max_bytes
        && actual.max_messages == desired.max_messages
        && actual.max_messages_per_subject == desired.max_messages_per_subject
        && actual.discard == desired.discard
        && actual.discard_new_per_subject == desired.discard_new_per_subject
        && actual.retention == desired.retention
        && actual.max_consumers == desired.max_consumers
        && actual.max_age == desired.max_age
        && actual.max_message_size == desired.max_message_size
        && actual.duplicate_window == desired.duplicate_window
        && actual.storage == desired.storage
        && actual.num_replicas == desired.num_replicas
        && actual.no_ack == desired.no_ack
        && actual.template_owner == desired.template_owner
        && actual.sealed == desired.sealed
        && actual.allow_rollup == desired.allow_rollup
        && actual.deny_delete == desired.deny_delete
        && actual.deny_purge == desired.deny_purge
        && actual.republish == desired.republish
        && actual.allow_direct == desired.allow_direct
        && actual.mirror_direct == desired.mirror_direct
        && actual.mirror == desired.mirror
        && actual.sources == desired.sources
        && actual.metadata == desired.metadata
        && actual.subject_transform == desired.subject_transform
        && actual.compression == desired.compression
        && actual.consumer_limits == desired.consumer_limits
        && actual.first_sequence == desired.first_sequence
        && actual.placement == desired.placement
        && actual.persist_mode == desired.persist_mode
}

fn event_envelope(
    row: &OutboxClaim,
) -> Result<EventEnvelopeWire<serde_json::Value>, JetStreamOutboxError> {
    let authenticated_actor_origin = row.authenticated_actor_origin.0.clone();
    let occurred_at_unix_ms = u64::try_from(row.recorded_at.timestamp_millis())
        .map_err(|_| JetStreamOutboxError::InvalidOutboxPayload)?;
    Ok(EventEnvelopeWire {
        schema_version: EVENT_ENVELOPE_WIRE_SCHEMA_VERSION,
        event_schema_version: u32::try_from(row.event_schema_version)
            .map_err(|_| JetStreamOutboxError::InvalidOutboxPayload)?,
        sequence: u64::try_from(row.event_sequence)
            .map_err(|_| JetStreamOutboxError::InvalidOutboxPayload)?,
        stream_id: row.stream_id.clone(),
        stream_version: u64::try_from(row.stream_version)
            .map_err(|_| JetStreamOutboxError::InvalidOutboxPayload)?,
        event_type: row.event_type.clone(),
        campaign_id: row.campaign_id.clone(),
        authenticated_actor_id: row.authenticated_actor_id.clone(),
        authenticated_actor_role: row.authenticated_actor_role.clone(),
        authenticated_actor_origin,
        resource_campaign_id: row.campaign_id.clone(),
        resource_type: row.resource_type.clone(),
        resource_id: row.resource_id.clone(),
        authority_contract_id: row.authority_contract_id.clone(),
        authority_owner: row.authority_owner.clone(),
        command_id: row.command_id.clone(),
        idempotency_key: row.event_idempotency_key.clone(),
        authority_contract_version: u64::try_from(row.authority_contract_version)
            .map_err(|_| JetStreamOutboxError::InvalidOutboxPayload)?,
        visibility_label: row.visibility_label.clone(),
        visibility_subject: (row.visibility_subject != "not_applicable")
            .then(|| row.visibility_subject.clone()),
        provenance_kind: row.provenance_kind.clone(),
        provenance_reference: row.provenance_reference.clone(),
        provenance_recorded_by: row.provenance_recorded_by.clone(),
        correlation_id: row.correlation_id.clone(),
        causation_id: row.causation_id.clone(),
        trace_id: row.trace_id.clone(),
        occurred_at_unix_ms,
        payload: row.payload_json.clone(),
        request_hash_source: row.request_hash_source.clone(),
        integrity_status: row.integrity_status.clone(),
        integrity_hash: row.event_integrity_hash.clone(),
    })
}

fn validate_worker_id(worker_id: &str) -> Result<(), JetStreamOutboxError> {
    if worker_id.trim().is_empty()
        || worker_id.len() > 128
        || !worker_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
    {
        Err(JetStreamOutboxError::Configuration("invalid_worker_id"))
    } else {
        Ok(())
    }
}

fn map_worker_error(error: EventWorkerError) -> JetStreamOutboxError {
    match error {
        EventWorkerError::Configuration(reason) => JetStreamOutboxError::Configuration(reason),
        EventWorkerError::InvalidOutboxPayload => JetStreamOutboxError::InvalidOutboxPayload,
        EventWorkerError::ClaimLost => JetStreamOutboxError::Database("outbox_claim_lost"),
        EventWorkerError::Database(operation) => JetStreamOutboxError::Database(operation),
        EventWorkerError::ProjectionHash(_)
        | EventWorkerError::ProjectionSerialization
        | EventWorkerError::ProjectionReadModelConflict
        | EventWorkerError::ProjectionStreamGap { .. }
        | EventWorkerError::CheckpointIdentityMismatch
        | EventWorkerError::CheckpointConflict { .. } => {
            JetStreamOutboxError::Database("unexpected_projection_worker_error")
        }
    }
}

fn validate_nats_url(nats_url: &str) -> Result<(bool, bool), JetStreamOutboxError> {
    let url = Url::parse(nats_url)
        .map_err(|_| JetStreamOutboxError::Configuration("invalid_nats_url"))?;
    let host = url
        .host_str()
        .ok_or(JetStreamOutboxError::Configuration("nats_host_required"))?;
    let local = matches!(host, "localhost" | "127.0.0.1" | "::1");
    let tls = url.scheme() == "tls";
    if !matches!(url.scheme(), "nats" | "tls") {
        return Err(JetStreamOutboxError::Configuration(
            "unsupported_nats_scheme",
        ));
    }
    if !local && !tls {
        return Err(JetStreamOutboxError::Configuration(
            "remote_nats_requires_tls",
        ));
    }
    Ok((local, tls))
}

fn nats_url_credentials(nats_url: &str) -> Result<Option<(String, String)>, JetStreamOutboxError> {
    let url = Url::parse(nats_url)
        .map_err(|_| JetStreamOutboxError::Configuration("invalid_nats_url"))?;
    match (url.username(), url.password()) {
        ("", None) => Ok(None),
        (username, Some(password)) if !username.is_empty() && !password.is_empty() => {
            let username = percent_decode_str(username)
                .decode_utf8()
                .map_err(|_| {
                    JetStreamOutboxError::Configuration("invalid_nats_url_credentials_encoding")
                })?
                .into_owned();
            let password = percent_decode_str(password)
                .decode_utf8()
                .map_err(|_| {
                    JetStreamOutboxError::Configuration("invalid_nats_url_credentials_encoding")
                })?
                .into_owned();
            if username.is_empty() || password.is_empty() {
                return Err(JetStreamOutboxError::Configuration(
                    "nats_url_credentials_incomplete",
                ));
            }
            Ok(Some((username, password)))
        }
        _ => Err(JetStreamOutboxError::Configuration(
            "nats_url_credentials_incomplete",
        )),
    }
}

fn nats_endpoint_without_userinfo(nats_url: &str) -> Result<String, JetStreamOutboxError> {
    let mut url = Url::parse(nats_url)
        .map_err(|_| JetStreamOutboxError::Configuration("invalid_nats_url"))?;
    url.set_username("")
        .map_err(|_| JetStreamOutboxError::Configuration("invalid_nats_url"))?;
    url.set_password(None)
        .map_err(|_| JetStreamOutboxError::Configuration("invalid_nats_url"))?;
    Ok(url.into())
}

#[cfg(test)]
fn outbox_integrity_metadata_is_valid(
    integrity_status: &str,
    request_hash_source: &str,
    has_integrity_hash: bool,
    has_commit_id: bool,
) -> bool {
    match (integrity_status, request_hash_source) {
        ("verified_hmac", "formal_commit") => has_integrity_hash && has_commit_id,
        _ => false,
    }
}

fn insert_outbox_header(
    headers: &mut HeaderMap,
    name: &'static str,
    value: &str,
) -> Result<(), JetStreamOutboxError> {
    // async-nats' infallible `From<&str>` implementation asserts on CR/LF.
    // Historical rows can predate today's database validators, so parse every
    // dynamic value through the fallible API and keep failure row-scoped.
    let value =
        HeaderValue::from_str(value).map_err(|_| JetStreamOutboxError::InvalidOutboxPayload)?;
    headers.insert(name, value);
    Ok(())
}

fn outbox_headers(row: &OutboxClaim) -> Result<HeaderMap, JetStreamOutboxError> {
    let mut headers = HeaderMap::new();
    // JetStream duplicate detection is global to the NATS stream. Bind its
    // message id to the complete persisted idempotency scope so equal client
    // keys in distinct campaign/resource streams cannot suppress one another.
    insert_outbox_header(&mut headers, "Nats-Msg-Id", &nats_message_id(row))?;
    insert_outbox_header(&mut headers, "Trpg-Idempotency-Key", &row.idempotency_key)?;
    insert_outbox_header(&mut headers, "Trpg-Stream-Id", &row.stream_id)?;
    insert_outbox_header(
        &mut headers,
        "Trpg-Idempotency-Operation",
        &row.idempotency_operation,
    )?;
    if let Some(commit_id) = &row.commit_id {
        insert_outbox_header(&mut headers, "Trpg-Commit-Id", commit_id)?;
    }
    insert_outbox_header(&mut headers, "Trpg-Correlation-Id", &row.correlation_id)?;
    insert_outbox_header(&mut headers, "Trpg-Visibility", &row.visibility_label)?;
    insert_outbox_header(
        &mut headers,
        "Trpg-Data-Subject-Digest",
        &data_subject_digest(&row.data_subject_id),
    )?;
    insert_outbox_header(&mut headers, "Trpg-Integrity-Status", &row.integrity_status)?;
    insert_outbox_header(
        &mut headers,
        "Trpg-Request-Hash-Source",
        &row.request_hash_source,
    )?;
    Ok(headers)
}

fn data_subject_digest(data_subject_id: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(data_subject_id.as_bytes()))
}

fn canonical_delivery_subject(row: &OutboxClaim) -> String {
    if row.data_subject_id == "not_applicable" {
        format!("{}.unscoped", row.subject)
    } else {
        format!(
            "{}.subject.{:x}",
            row.subject,
            Sha256::digest(row.data_subject_id.as_bytes())
        )
    }
}

fn nats_message_id(row: &OutboxClaim) -> String {
    let mut digest = Sha256::new();
    for field in [
        row.campaign_id.as_str(),
        row.stream_id.as_str(),
        row.idempotency_operation.as_str(),
        row.idempotency_key.as_str(),
    ] {
        digest.update((field.len() as u64).to_be_bytes());
        digest.update(field.as_bytes());
    }
    format!("trpg-outbox-sha256:{:x}", digest.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claimed_row(integrity_status: &str, request_hash_source: &str) -> OutboxClaim {
        let claimed_at = chrono::Utc::now();
        OutboxClaim {
            outbox_id: 1,
            event_sequence: 1,
            subject: "trpg.events.appended".to_owned(),
            idempotency_key: "claimed_row".to_owned(),
            visibility_label: "party_visible".to_owned(),
            correlation_id: "correlation".to_owned(),
            causation_id: "causation".to_owned(),
            payload_json: serde_json::json!({}),
            commit_id: None,
            event_type: "ClaimedRowProbe".to_owned(),
            event_schema_version: 1,
            campaign_id: "campaign".to_owned(),
            stream_id: "campaign".to_owned(),
            stream_version: 1,
            expected_version: 0,
            event_idempotency_key: "event_claimed_row".to_owned(),
            idempotency_operation: "canonical_commit".to_owned(),
            authenticated_actor_id: "historical_import".to_owned(),
            authenticated_actor_role: "historical_unknown".to_owned(),
            authenticated_actor_origin: sqlx::types::Json(EventActorOriginWire::Workload {
                role: "historical_unknown".to_owned(),
            }),
            resource_type: "campaign".to_owned(),
            resource_id: "campaign".to_owned(),
            authority_contract_id: "historical_authority".to_owned(),
            authority_owner: "historical_owner".to_owned(),
            authority_contract_version: 1,
            command_id: "historical_command".to_owned(),
            visibility_subject: "not_applicable".to_owned(),
            data_subject_id: "not_applicable".to_owned(),
            provenance_kind: "rules_engine_decision".to_owned(),
            provenance_reference: "decision".to_owned(),
            provenance_recorded_by: "rules_engine".to_owned(),
            trace_id: "historical_trace".to_owned(),
            recorded_at: claimed_at,
            event_integrity_hash: None,
            request_hash: "sha256:0000000000000000000000000000000000000000000000000000000000000000"
                .to_owned(),
            request_hash_source: request_hash_source.to_owned(),
            integrity_status: integrity_status.to_owned(),
            payload_integrity_source: "{}".to_owned(),
            retry_count: 0,
            claimed_at,
            locked_until: claimed_at + chrono::Duration::seconds(60),
            claim_token: "claim-sha256:test:1".to_owned(),
        }
    }

    #[test]
    fn dead_letters_raise_a_stable_operator_alert() {
        let healthy = PublishBatchResult::default();
        assert!(!healthy.requires_operator_attention());
        assert_eq!(healthy.alert_code(), None);
        let failed = PublishBatchResult {
            claimed: 1,
            failed: 1,
            dead_lettered: 1,
            ..PublishBatchResult::default()
        };
        assert!(failed.requires_operator_attention());
        assert_eq!(failed.alert_code(), Some("OUTBOX_DEAD_LETTER_ALERT"));

        let persisted = PublishBatchResult {
            dead_letter_total: 1,
            ..PublishBatchResult::default()
        };
        assert!(persisted.requires_operator_attention());
        assert_eq!(persisted.alert_code(), Some("OUTBOX_DEAD_LETTER_ALERT"));
    }

    #[test]
    fn remote_plaintext_nats_is_rejected() {
        assert_eq!(
            validate_nats_url("nats://nats.example.invalid:4222"),
            Err(JetStreamOutboxError::Configuration(
                "remote_nats_requires_tls"
            ))
        );
        assert_eq!(
            validate_nats_url("tls://nats.example.invalid:4222"),
            Ok((false, true))
        );
        assert_eq!(
            nats_url_credentials("tls://runtime:secret@nats.example.invalid:4222"),
            Ok(Some(("runtime".to_owned(), "secret".to_owned())))
        );
        assert_eq!(
            nats_url_credentials("tls://runtime%20user:secret%40value@nats.example.invalid:4222"),
            Ok(Some(("runtime user".to_owned(), "secret@value".to_owned())))
        );
        assert_eq!(
            nats_endpoint_without_userinfo(
                "tls://runtime%20user:secret%40value@nats.example.invalid:4222"
            ),
            Ok("tls://nats.example.invalid:4222".to_owned())
        );
        assert_eq!(
            nats_url_credentials("tls://runtime@nats.example.invalid:4222"),
            Err(JetStreamOutboxError::Configuration(
                "nats_url_credentials_incomplete"
            ))
        );
    }

    #[test]
    fn outbox_integrity_metadata_rejects_mixed_states() {
        assert!(outbox_integrity_metadata_is_valid(
            "verified_hmac",
            "formal_commit",
            true,
            true,
        ));
        assert!(!outbox_integrity_metadata_is_valid(
            "historical_unsigned",
            "historical_unavailable",
            false,
            false,
        ));
        assert!(!outbox_integrity_metadata_is_valid(
            "historical_unverified_hmac",
            "formal_commit",
            true,
            true,
        ));
        assert!(!outbox_integrity_metadata_is_valid(
            "verified_hmac",
            "historical_unavailable",
            true,
            false,
        ));
        assert!(!outbox_integrity_metadata_is_valid(
            "historical_unsigned",
            "formal_commit",
            false,
            true,
        ));
    }

    #[test]
    fn claimed_row_integrity_failure_is_scoped_to_its_delivery_attempt() {
        let invalid = claimed_row("verified_hmac", "historical_unavailable");
        assert_eq!(
            invalid.validate_for_publish(),
            Err(EventWorkerError::InvalidOutboxPayload)
        );

        let historical = claimed_row("historical_unsigned", "historical_unavailable");
        assert_eq!(
            historical.validate_for_publish(),
            Err(EventWorkerError::InvalidOutboxPayload)
        );
    }

    #[test]
    fn malformed_historical_header_values_return_errors_instead_of_panicking() {
        let valid = claimed_row("historical_unsigned", "historical_unavailable");
        assert!(outbox_headers(&valid).is_ok());

        let mut bad_idempotency = valid.clone();
        bad_idempotency.idempotency_key = "historic\r\nmessage-id".to_owned();
        assert!(matches!(
            outbox_headers(&bad_idempotency),
            Err(JetStreamOutboxError::InvalidOutboxPayload)
        ));

        let mut bad_correlation = valid.clone();
        bad_correlation.correlation_id = "historic\ncorrelation".to_owned();
        assert!(matches!(
            outbox_headers(&bad_correlation),
            Err(JetStreamOutboxError::InvalidOutboxPayload)
        ));

        let mut bad_commit = valid;
        bad_commit.commit_id = Some("historic\rcommit".to_owned());
        assert!(matches!(
            outbox_headers(&bad_commit),
            Err(JetStreamOutboxError::InvalidOutboxPayload)
        ));
    }

    #[test]
    fn jetstream_message_id_binds_the_complete_idempotency_scope() {
        let first = claimed_row("historical_unsigned", "historical_unavailable");
        let mut other_stream = first.clone();
        other_stream.stream_id = "other_stream".to_owned();
        assert_ne!(nats_message_id(&first), nats_message_id(&other_stream));
        assert_eq!(nats_message_id(&first), nats_message_id(&first.clone()));
    }

    #[test]
    fn production_event_envelope_is_versioned_and_never_forges_historical_integrity() {
        let historical = claimed_row("historical_unsigned", "historical_unavailable");
        assert_eq!(
            historical.validate_for_publish(),
            Err(EventWorkerError::InvalidOutboxPayload)
        );

        let mut formal = historical;
        formal.commit_id = Some("formal_commit".to_owned());
        formal.request_hash_source = "formal_commit".to_owned();
        formal.integrity_status = "verified_hmac".to_owned();
        formal.event_integrity_hash = Some(
            "hmac-sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .to_owned(),
        );
        formal.authenticated_actor_id = "keeper".to_owned();
        formal.authenticated_actor_role = "human_keeper".to_owned();
        formal.authenticated_actor_origin = sqlx::types::Json(EventActorOriginWire::UserSession {
            session_id: "session".to_owned(),
        });
        let cipher = PayloadCipher::new("outbox-test-key", &[0x42; 32]).unwrap();
        let encrypted = cipher
            .encrypt_json_field(
                br#"{"clue":"harbor ledger"}"#,
                &[
                    &formal.campaign_id,
                    &formal.stream_id,
                    &formal.command_id,
                    &formal.event_type,
                ],
            )
            .unwrap();
        formal.payload_json = encrypted.envelope().clone();
        formal.payload_integrity_source = serde_json::to_string(encrypted.envelope()).unwrap();
        formal.validate_for_publish().unwrap();
        let envelope = event_envelope(&formal).unwrap();
        assert_eq!(envelope.schema_version, EVENT_ENVELOPE_WIRE_SCHEMA_VERSION);
        assert_eq!(envelope.event_schema_version, 1);
        assert!(matches!(
            envelope.authenticated_actor_origin,
            EventActorOriginWire::UserSession { ref session_id } if session_id == "session"
        ));
        assert_eq!(envelope.request_hash_source, "formal_commit");
        assert_eq!(envelope.integrity_status, "verified_hmac");
        assert_eq!(envelope.integrity_hash, formal.event_integrity_hash);
        assert!(envelope.payload.get("protected_payload").is_some());
    }

    #[test]
    fn publisher_forwards_only_the_protected_payload_envelope() {
        let cipher = PayloadCipher::new("outbox-test-key", &[0x42; 32]).unwrap();
        let mut formal = claimed_row("verified_hmac", "formal_commit");
        formal.commit_id = Some("formal_commit".to_owned());
        formal.event_integrity_hash = Some(
            "hmac-sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .to_owned(),
        );
        let encrypted = cipher
            .encrypt_json_field(
                br#"{"clue":"harbor ledger"}"#,
                &[
                    &formal.campaign_id,
                    &formal.stream_id,
                    &formal.command_id,
                    &formal.event_type,
                ],
            )
            .unwrap();
        formal.payload_json = encrypted.envelope().clone();
        formal.payload_integrity_source = serde_json::to_string(encrypted.envelope()).unwrap();

        formal.validate_for_publish().unwrap();
        let bytes = serde_json::to_vec(&event_envelope(&formal).unwrap()).unwrap();
        let wire: EventEnvelopeWire<serde_json::Value> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(wire.payload, formal.payload_json);
        assert!(wire.payload.get("protected_payload").is_some());
        assert!(!String::from_utf8(bytes).unwrap().contains("harbor ledger"));
    }

    #[test]
    fn canonical_delivery_subject_is_data_subject_scoped() {
        let mut claim = claimed_row("verified_hmac", "formal_commit");
        assert_eq!(
            canonical_delivery_subject(&claim),
            "trpg.events.appended.unscoped"
        );
        claim.data_subject_id = "player_subject_123".to_owned();
        assert_eq!(
            canonical_delivery_subject(&claim),
            format!(
                "trpg.events.appended.subject.{:x}",
                Sha256::digest(b"player_subject_123")
            )
        );
    }

    #[test]
    fn every_configured_jetstream_safety_field_is_fail_closed() {
        let desired = canonical_stream_config();
        assert!(stream_config_matches(&desired, &desired));

        let mut variants = Vec::new();
        let mut changed = desired.clone();
        changed.subjects = vec!["trpg.events.appended".to_owned()];
        variants.push(changed);
        let mut changed = desired.clone();
        changed.max_bytes -= 1;
        variants.push(changed);
        let mut changed = desired.clone();
        changed.max_age -= Duration::from_secs(1);
        variants.push(changed);
        let mut changed = desired.clone();
        changed.duplicate_window -= Duration::from_secs(1);
        variants.push(changed);
        let mut changed = desired.clone();
        changed.storage = StorageType::Memory;
        variants.push(changed);
        let mut changed = desired.clone();
        changed.num_replicas = 2;
        variants.push(changed);
        let mut changed = desired.clone();
        changed.no_ack = true;
        variants.push(changed);
        let mut changed = desired.clone();
        changed.deny_delete = true;
        variants.push(changed);
        let mut changed = desired.clone();
        changed.deny_purge = false;
        variants.push(changed);
        let mut changed = desired.clone();
        changed.retention = async_nats::jetstream::stream::RetentionPolicy::WorkQueue;
        variants.push(changed);
        let mut changed = desired.clone();
        changed
            .metadata
            .insert("owner".to_owned(), "unexpected".to_owned());
        variants.push(changed);
        let mut changed = desired.clone();
        changed.subject_transform = Some(async_nats::jetstream::stream::SubjectTransform {
            source: "trpg.events.>".to_owned(),
            destination: "transformed.>".to_owned(),
        });
        variants.push(changed);
        let mut changed = desired.clone();
        changed.compression = Some(async_nats::jetstream::stream::Compression::S2);
        variants.push(changed);
        let mut changed = desired.clone();
        changed.consumer_limits = Some(async_nats::jetstream::stream::ConsumerLimits {
            inactive_threshold: Duration::from_secs(60),
            max_ack_pending: 32,
        });
        variants.push(changed);
        let mut changed = desired.clone();
        changed.first_sequence = Some(2);
        variants.push(changed);

        assert!(variants
            .iter()
            .all(|actual| !stream_config_matches(actual, &desired)));
    }
}
