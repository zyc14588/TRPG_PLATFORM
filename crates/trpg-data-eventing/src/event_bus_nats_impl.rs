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

pub const OUTBOX_FLOW_STATES: &[&str] = &["pending", "published", "retrying", "dead_lettered"];
pub const PUBLISH_SOURCE: &str = crate::OUTBOX_TABLE;

use std::fmt;
use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use async_nats::jetstream::stream::{Config as StreamConfig, StorageType};
use async_nats::{ConnectOptions, HeaderMap};
use serde_json::{json, Value};
use sqlx::postgres::{PgConnectOptions, PgPoolOptions, PgSslMode};
use sqlx::{PgPool, Row};
use url::Url;

const STREAM_NAME: &str = "TRPG_CANONICAL_EVENTS";
const MAX_PUBLISH_RETRIES: i32 = 10;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PublishBatchResult {
    pub claimed: usize,
    pub published: usize,
    pub failed: usize,
    pub dead_lettered: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum JetStreamOutboxError {
    Configuration(&'static str),
    PostgresUnavailable,
    NatsUnavailable,
    StreamUnavailable,
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
            Self::Database(operation) => write!(formatter, "outbox database failed: {operation}"),
            Self::InvalidOutboxPayload => formatter.write_str("invalid outbox payload"),
        }
    }
}

impl std::error::Error for JetStreamOutboxError {}

#[derive(Clone)]
pub struct JetStreamOutboxPublisher {
    pool: PgPool,
    jetstream: async_nats::jetstream::Context,
    worker_id: String,
    batch_size: i64,
}

impl fmt::Debug for JetStreamOutboxPublisher {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JetStreamOutboxPublisher")
            .field("pool", &"[POSTGRESQL POOL]")
            .field("jetstream", &"[JETSTREAM CONTEXT]")
            .field("worker_id", &self.worker_id)
            .field("batch_size", &self.batch_size)
            .finish()
    }
}

#[derive(Debug)]
struct ClaimedOutboxRow {
    outbox_id: i64,
    event_sequence: i64,
    subject: String,
    idempotency_key: String,
    visibility_label: String,
    correlation_id: String,
    causation_id: String,
    payload_json: String,
    commit_id: String,
    event_type: String,
    campaign_id: String,
    stream_version: i64,
    visibility_subject: String,
    provenance_kind: String,
    provenance_reference: String,
    provenance_recorded_by: String,
    event_integrity_hash: String,
    retry_count: i32,
}

impl JetStreamOutboxPublisher {
    pub async fn connect(
        database_url: &str,
        nats_url: &str,
        worker_id: &str,
        nats_ca_certificate_path: Option<&Path>,
    ) -> Result<Self, JetStreamOutboxError> {
        Self::connect_with_credentials(
            database_url,
            nats_url,
            worker_id,
            nats_ca_certificate_path,
            None,
        )
        .await
    }

    pub async fn connect_with_credentials(
        database_url: &str,
        nats_url: &str,
        worker_id: &str,
        nats_ca_certificate_path: Option<&Path>,
        nats_credentials_path: Option<&Path>,
    ) -> Result<Self, JetStreamOutboxError> {
        validate_worker_id(worker_id)?;
        let database_options = PgConnectOptions::from_str(database_url)
            .map_err(|_| JetStreamOutboxError::Configuration("invalid_postgresql_url"))?;
        let database_host = database_options.get_host();
        let local_database = matches!(database_host, "localhost" | "127.0.0.1" | "::1")
            || database_host.starts_with('/');
        if !local_database && !matches!(database_options.get_ssl_mode(), PgSslMode::VerifyFull) {
            return Err(JetStreamOutboxError::Configuration(
                "remote_postgresql_requires_sslmode_verify_full",
            ));
        }
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect_with(database_options)
            .await
            .map_err(|_| JetStreamOutboxError::PostgresUnavailable)?;

        let (local_nats, tls_nats) = validate_nats_url(nats_url)?;
        if !local_nats && nats_credentials_path.is_none() {
            return Err(JetStreamOutboxError::Configuration(
                "remote_nats_credentials_required",
            ));
        }
        let mut options = ConnectOptions::new()
            .name(worker_id)
            .require_tls(tls_nats || !local_nats)
            .connection_timeout(Duration::from_secs(5));
        if let Some(path) = nats_ca_certificate_path {
            options = options.add_root_certificates(path.to_path_buf());
        }
        if let Some(path) = nats_credentials_path {
            options = options
                .credentials_file(path)
                .await
                .map_err(|_| JetStreamOutboxError::Configuration("invalid_nats_credentials"))?;
        }
        let client = options
            .connect(nats_url)
            .await
            .map_err(|_| JetStreamOutboxError::NatsUnavailable)?;
        Ok(Self {
            pool,
            jetstream: async_nats::jetstream::new(client),
            worker_id: worker_id.to_owned(),
            batch_size: 100,
        })
    }

    pub async fn ensure_stream(&self) -> Result<(), JetStreamOutboxError> {
        let mut stream = self
            .jetstream
            .get_or_create_stream(StreamConfig {
                name: STREAM_NAME.to_owned(),
                description: Some("Canonical TRPG event outbox".to_owned()),
                subjects: vec!["trpg.events.>".to_owned()],
                max_bytes: 10 * 1024 * 1024 * 1024,
                max_age: Duration::from_secs(7 * 24 * 60 * 60),
                duplicate_window: Duration::from_secs(120),
                storage: StorageType::File,
                num_replicas: 1,
                deny_delete: true,
                deny_purge: true,
                ..Default::default()
            })
            .await
            .map_err(|_| JetStreamOutboxError::StreamUnavailable)?;
        let info = stream
            .info()
            .await
            .map_err(|_| JetStreamOutboxError::StreamUnavailable)?;
        if info.config.storage != StorageType::File
            || !info
                .config
                .subjects
                .iter()
                .any(|subject| subject == "trpg.events.>")
            || info.config.no_ack
        {
            return Err(JetStreamOutboxError::Configuration(
                "jetstream_stream_contract_mismatch",
            ));
        }
        Ok(())
    }

    pub async fn check_readiness(&self) -> Result<(), JetStreamOutboxError> {
        self.ensure_stream().await
    }

    pub async fn publish_batch(&self) -> Result<PublishBatchResult, JetStreamOutboxError> {
        let claimed = self.claim_batch().await?;
        let mut result = PublishBatchResult {
            claimed: claimed.len(),
            ..PublishBatchResult::default()
        };
        for row in claimed {
            match self.publish_one(&row).await {
                Ok(()) => {
                    self.mark_published(&row).await?;
                    result.published += 1;
                }
                Err(_) => {
                    let dead_lettered = self.mark_failed(&row).await?;
                    result.failed += 1;
                    if dead_lettered {
                        result.dead_lettered += 1;
                    }
                }
            }
        }
        Ok(result)
    }

    pub async fn pending_count(&self) -> Result<i64, JetStreamOutboxError> {
        sqlx::query_scalar(
            "SELECT count(*) FROM event_outbox WHERE published_at IS NULL AND dead_lettered_at IS NULL",
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|_| JetStreamOutboxError::Database("count_pending"))
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

    async fn claim_batch(&self) -> Result<Vec<ClaimedOutboxRow>, JetStreamOutboxError> {
        let rows = sqlx::query(
            r#"
            WITH candidates AS (
                SELECT outbox.outbox_id, event.event_type, event.campaign_id,
                       event.stream_version, event.visibility_subject,
                       event.fact_provenance_kind, event.fact_provenance_reference,
                       event.fact_recorded_by, event.event_integrity_hash
                  FROM event_outbox outbox
                  JOIN event_store event ON event.sequence = outbox.event_sequence
                 WHERE outbox.published_at IS NULL
                   AND outbox.dead_lettered_at IS NULL
                   AND (
                       outbox.claimed_at IS NULL
                       OR outbox.claimed_at < now() - interval '60 seconds'
                   )
                 ORDER BY outbox.outbox_id
                 FOR UPDATE OF outbox SKIP LOCKED
                 LIMIT $2
            )
            UPDATE event_outbox outbox
               SET claimed_at = now(), claim_owner = $1
              FROM candidates
             WHERE outbox.outbox_id = candidates.outbox_id
            RETURNING outbox.outbox_id, outbox.event_sequence, outbox.nats_subject,
                      outbox.idempotency_key, outbox.visibility_label,
                      outbox.correlation_id, outbox.causation_id, outbox.payload_json,
                      outbox.commit_id, outbox.retry_count,
                      candidates.event_type, candidates.campaign_id,
                      candidates.stream_version, candidates.visibility_subject,
                      candidates.fact_provenance_kind, candidates.fact_provenance_reference,
                      candidates.fact_recorded_by, candidates.event_integrity_hash
            "#,
        )
        .bind(&self.worker_id)
        .bind(self.batch_size)
        .fetch_all(&self.pool)
        .await
        .map_err(|_| JetStreamOutboxError::Database("claim_batch"))?;
        rows.iter()
            .map(|row| {
                let event_integrity_hash: Option<String> = row.get("event_integrity_hash");
                let commit_id: Option<String> = row.get("commit_id");
                Ok(ClaimedOutboxRow {
                    outbox_id: row.get("outbox_id"),
                    event_sequence: row.get("event_sequence"),
                    subject: row.get("nats_subject"),
                    idempotency_key: row.get("idempotency_key"),
                    visibility_label: row.get("visibility_label"),
                    correlation_id: row.get("correlation_id"),
                    causation_id: row.get("causation_id"),
                    payload_json: row.get("payload_json"),
                    commit_id: commit_id.ok_or(JetStreamOutboxError::InvalidOutboxPayload)?,
                    event_type: row.get("event_type"),
                    campaign_id: row.get("campaign_id"),
                    stream_version: row.get("stream_version"),
                    visibility_subject: row.get("visibility_subject"),
                    provenance_kind: row.get("fact_provenance_kind"),
                    provenance_reference: row.get("fact_provenance_reference"),
                    provenance_recorded_by: row.get("fact_recorded_by"),
                    event_integrity_hash: event_integrity_hash
                        .ok_or(JetStreamOutboxError::InvalidOutboxPayload)?,
                    retry_count: row.get("retry_count"),
                })
            })
            .collect()
    }

    async fn publish_one(&self, row: &ClaimedOutboxRow) -> Result<(), JetStreamOutboxError> {
        let payload: Value = serde_json::from_str(&row.payload_json)
            .map_err(|_| JetStreamOutboxError::InvalidOutboxPayload)?;
        let envelope = serde_json::to_vec(&json!({
            "event_sequence": row.event_sequence,
            "stream_version": row.stream_version,
            "event_type": row.event_type,
            "commit_id": row.commit_id,
            "campaign_id": row.campaign_id,
            "visibility_label": row.visibility_label,
            "visibility_subject": row.visibility_subject,
            "provenance_kind": row.provenance_kind,
            "provenance_reference": row.provenance_reference,
            "provenance_recorded_by": row.provenance_recorded_by,
            "correlation_id": row.correlation_id,
            "causation_id": row.causation_id,
            "event_integrity_hash": row.event_integrity_hash,
            "payload": payload,
        }))
        .map_err(|_| JetStreamOutboxError::InvalidOutboxPayload)?;
        let mut headers = HeaderMap::new();
        headers.insert("Nats-Msg-Id", row.idempotency_key.as_str());
        headers.insert("Trpg-Commit-Id", row.commit_id.as_str());
        headers.insert("Trpg-Correlation-Id", row.correlation_id.as_str());
        headers.insert("Trpg-Visibility", row.visibility_label.as_str());
        self.jetstream
            .publish_with_headers(row.subject.clone(), headers, envelope.into())
            .await
            .map_err(|_| JetStreamOutboxError::NatsUnavailable)?
            .await
            .map_err(|_| JetStreamOutboxError::NatsUnavailable)?;
        Ok(())
    }

    async fn mark_published(&self, row: &ClaimedOutboxRow) -> Result<(), JetStreamOutboxError> {
        let result = sqlx::query(
            r#"
            UPDATE event_outbox
               SET published_at = now(), claimed_at = NULL, claim_owner = NULL,
                   last_error = NULL
             WHERE outbox_id = $1 AND claim_owner = $2 AND published_at IS NULL
            "#,
        )
        .bind(row.outbox_id)
        .bind(&self.worker_id)
        .execute(&self.pool)
        .await
        .map_err(|_| JetStreamOutboxError::Database("mark_published"))?;
        if result.rows_affected() == 1 {
            Ok(())
        } else {
            Err(JetStreamOutboxError::Database("publish_claim_lost"))
        }
    }

    async fn mark_failed(&self, row: &ClaimedOutboxRow) -> Result<bool, JetStreamOutboxError> {
        let dead_letter = row.retry_count.saturating_add(1) >= MAX_PUBLISH_RETRIES;
        let result = sqlx::query(
            r#"
            UPDATE event_outbox
               SET retry_count = retry_count + 1,
                   last_error = 'JETSTREAM_PUBLISH_FAILED',
                   claimed_at = NULL,
                   claim_owner = NULL,
                   dead_lettered_at = CASE WHEN $3 THEN now() ELSE dead_lettered_at END
             WHERE outbox_id = $1 AND claim_owner = $2 AND published_at IS NULL
            "#,
        )
        .bind(row.outbox_id)
        .bind(&self.worker_id)
        .bind(dead_letter)
        .execute(&self.pool)
        .await
        .map_err(|_| JetStreamOutboxError::Database("mark_failed"))?;
        if result.rows_affected() == 1 {
            Ok(dead_letter)
        } else {
            Err(JetStreamOutboxError::Database("failure_claim_lost"))
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

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
    }
}
