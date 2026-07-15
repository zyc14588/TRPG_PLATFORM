use crate::command_cqrs_idempotency::append_idempotent_event;
use crate::ddd::{CommandEnvelope, DomainResult, EntityId, EventEnvelope, EventStore};

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct DomainEventPayload {
    pub event_id: EntityId,
    pub event_name: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProjectionSnapshot {
    pub event_count: usize,
    pub last_sequence: u64,
    pub projection_hash: String,
}

pub fn append_domain_event<T>(
    store: &mut EventStore<DomainEventPayload>,
    command: &CommandEnvelope<T>,
    event_id: impl Into<String>,
    event_name: &'static str,
) -> DomainResult<EventEnvelope<DomainEventPayload>> {
    append_idempotent_event(
        store,
        command,
        event_name,
        DomainEventPayload {
            event_id: EntityId::new(event_id)?,
            event_name,
        },
    )
}

pub fn rebuild_projection(events: &[EventEnvelope<DomainEventPayload>]) -> ProjectionSnapshot {
    let last_sequence = events.last().map(|event| event.sequence).unwrap_or(0);
    let mut hash = 0xcbf29ce484222325u64;

    for event in events {
        fold_bytes(&mut hash, event.event_type.as_bytes());
        fold_bytes(&mut hash, event.payload.event_id.as_str().as_bytes());
        fold_bytes(&mut hash, event.sequence.to_string().as_bytes());
    }

    ProjectionSnapshot {
        event_count: events.len(),
        last_sequence,
        projection_hash: format!("{hash:016x}"),
    }
}

fn fold_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x100000001b3);
    }
}
