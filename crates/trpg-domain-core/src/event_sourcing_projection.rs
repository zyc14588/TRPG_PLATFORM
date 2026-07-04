use crate::ddd::{CommandEnvelope, DomainResult, EventEnvelope, EventStore, PrincipalScope};
use crate::domain_event_sourcing_projection::{
    append_canon_event, rebuild_canon_projection, replay_visible_canon_events,
};
use crate::event_sourcing_snapshot_projection::{DomainEventPayload, ProjectionSnapshot};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanonEventWrite {
    pub event_id: String,
    pub event_name: &'static str,
}

impl CanonEventWrite {
    pub fn new(event_id: impl Into<String>, event_name: &'static str) -> Self {
        Self {
            event_id: event_id.into(),
            event_name,
        }
    }
}

pub fn append_rebuildable_canon_event<T>(
    store: &mut EventStore<DomainEventPayload>,
    command: &CommandEnvelope<T>,
    write: CanonEventWrite,
) -> DomainResult<EventEnvelope<DomainEventPayload>> {
    append_canon_event(store, command, write.event_id, write.event_name)
}

pub fn rebuild_projection_from_events(
    events: &[EventEnvelope<DomainEventPayload>],
) -> ProjectionSnapshot {
    rebuild_canon_projection(events)
}

pub fn replay_visible_projection_events(
    store: &EventStore<DomainEventPayload>,
    principal: &PrincipalScope,
) -> Vec<EventEnvelope<DomainEventPayload>> {
    replay_visible_canon_events(store, principal)
}
