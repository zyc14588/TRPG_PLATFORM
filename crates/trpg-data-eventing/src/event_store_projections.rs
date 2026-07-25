crate::define_data_event_module!(
    EventStoreProjectionsCommand,
    EventStoreProjectionsOperation,
    append_event_store_projections_event,
    "event_store_projections",
    "EventStoreProjectionRebuilt",
    "data_eventing.event_store_projections.event_schema",
    crate::DataEventOperation::ProjectionRebuild,
    ["projection_view", "replay_cursor"]
);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CanonicalProjectionRoute {
    pub event_name: &'static str,
    pub schema_version: u16,
    pub schema_id: &'static str,
    pub projection: &'static str,
}

pub fn canonical_projection_route(
    header: trpg_contracts::CanonicalEventHeader,
) -> CanonicalProjectionRoute {
    let descriptor = header.event_type.descriptor();
    CanonicalProjectionRoute {
        event_name: descriptor.name,
        schema_version: descriptor.schema_version,
        schema_id: descriptor.schema_id,
        projection: descriptor.projection,
    }
}

use sha2::{Digest, Sha256};

use crate::event_store_sqlx_outbox_projection::CanonicalReplayEvent;

pub const PROJECTION_HASH_GENESIS: &str =
    "sha256:0000000000000000000000000000000000000000000000000000000000000000";

/// Restartable hash chain for an Event Store-derived projection.
///
/// Every protected envelope field and the canonical JSON payload participates
/// in the next digest. Length-prefixed binary framing avoids delimiter and
/// platform encoding ambiguity, while hash chaining lets a worker resume from
/// its durable checkpoint without retaining an unbounded replay buffer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonicalProjectionHasher {
    projection_hash: String,
}

impl Default for CanonicalProjectionHasher {
    fn default() -> Self {
        Self {
            projection_hash: PROJECTION_HASH_GENESIS.to_owned(),
        }
    }
}

impl CanonicalProjectionHasher {
    pub fn resume(projection_hash: impl Into<String>) -> Result<Self, ProjectionHashError> {
        let projection_hash = projection_hash.into();
        if !is_sha256_hash(&projection_hash) {
            return Err(ProjectionHashError::InvalidCheckpointHash);
        }
        Ok(Self {
            projection_hash: projection_hash.to_ascii_lowercase(),
        })
    }

    pub fn apply(&mut self, event: &CanonicalReplayEvent) -> Result<&str, ProjectionHashError> {
        let payload = serde_json::to_vec(&event.payload)
            .map_err(|_| ProjectionHashError::PayloadSerialization)?;
        let actor_origin = serde_json::to_vec(&event.authenticated_actor_origin)
            .map_err(|_| ProjectionHashError::PayloadSerialization)?;
        let mut digest = Sha256::new();
        hash_field(&mut digest, 1, b"trpg-canonical-projection-hash-v3");
        hash_field(&mut digest, 2, self.projection_hash.as_bytes());
        hash_field(&mut digest, 3, &event.sequence.to_be_bytes());
        hash_field(&mut digest, 4, &event.stream_version.to_be_bytes());
        hash_field(&mut digest, 5, event.stream_id.as_bytes());
        hash_field(&mut digest, 6, event.event_type.as_bytes());
        hash_field(&mut digest, 7, &event.event_schema_version.to_be_bytes());
        hash_field(&mut digest, 8, event.campaign_id.as_bytes());
        hash_field(&mut digest, 9, &event.expected_version.to_be_bytes());
        hash_field(&mut digest, 10, event.authority_mode.as_bytes());
        hash_field(&mut digest, 11, event.authenticated_actor_id.as_bytes());
        hash_field(&mut digest, 36, event.authenticated_actor_role.as_bytes());
        hash_field(&mut digest, 37, &actor_origin);
        hash_field(&mut digest, 12, event.resource_type.as_bytes());
        hash_field(&mut digest, 13, event.resource_id.as_bytes());
        hash_field(&mut digest, 14, event.authority_contract_id.as_bytes());
        hash_field(&mut digest, 15, event.authority_owner.as_bytes());
        hash_field(&mut digest, 16, event.command_id.as_bytes());
        hash_field(&mut digest, 17, event.idempotency_key.as_bytes());
        hash_field(&mut digest, 18, event.idempotency_operation.as_bytes());
        hash_field(
            &mut digest,
            19,
            &event.authority_contract_version.to_be_bytes(),
        );
        hash_field(&mut digest, 20, event.visibility_label.as_bytes());
        hash_field(&mut digest, 21, event.visibility_subject.as_bytes());
        hash_field(&mut digest, 22, event.provenance_kind.as_bytes());
        hash_field(&mut digest, 23, event.provenance_reference.as_bytes());
        hash_field(&mut digest, 24, event.provenance_recorded_by.as_bytes());
        hash_field(&mut digest, 25, event.correlation_id.as_bytes());
        hash_field(&mut digest, 26, event.causation_id.as_bytes());
        hash_field(&mut digest, 27, event.trace_id.as_bytes());
        hash_field(
            &mut digest,
            29,
            &event.recorded_at.timestamp_micros().to_be_bytes(),
        );
        hash_field(
            &mut digest,
            30,
            event
                .event_integrity_hash
                .as_deref()
                .unwrap_or("")
                .as_bytes(),
        );
        hash_field(&mut digest, 31, event.request_hash.as_bytes());
        hash_field(&mut digest, 32, event.request_hash_source.as_bytes());
        hash_field(&mut digest, 33, event.integrity_status.as_bytes());
        hash_field(&mut digest, 34, event.payload_integrity_source.as_bytes());
        hash_field(&mut digest, 35, &payload);
        self.projection_hash = format!("sha256:{:x}", digest.finalize());
        Ok(&self.projection_hash)
    }

    pub fn projection_hash(&self) -> &str {
        &self.projection_hash
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectionHashError {
    InvalidCheckpointHash,
    PayloadSerialization,
}

impl std::fmt::Display for ProjectionHashError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidCheckpointHash => {
                formatter.write_str("invalid projection checkpoint hash")
            }
            Self::PayloadSerialization => {
                formatter.write_str("projection payload serialization failed")
            }
        }
    }
}

impl std::error::Error for ProjectionHashError {}

fn hash_field(digest: &mut Sha256, tag: u8, value: &[u8]) {
    digest.update([tag]);
    digest.update((value.len() as u64).to_be_bytes());
    digest.update(value);
}

fn is_sha256_hash(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
}
