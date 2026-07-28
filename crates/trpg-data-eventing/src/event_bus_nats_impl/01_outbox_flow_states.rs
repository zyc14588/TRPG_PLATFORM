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
