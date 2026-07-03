use crate::ddd::{CommandEnvelope, DomainResult, EventEnvelope, EventStore, PrincipalScope};
use crate::event_sourcing_snapshot_projection::{
    append_domain_event, rebuild_projection, DomainEventPayload, ProjectionSnapshot,
};

pub fn append_canon_event<T>(
    store: &mut EventStore<DomainEventPayload>,
    command: &CommandEnvelope<T>,
    event_id: impl Into<String>,
    event_name: &'static str,
) -> DomainResult<EventEnvelope<DomainEventPayload>> {
    append_domain_event(store, command, event_id, event_name)
}

pub fn rebuild_canon_projection(
    events: &[EventEnvelope<DomainEventPayload>],
) -> ProjectionSnapshot {
    rebuild_projection(events)
}

pub fn replay_visible_canon_events(
    store: &EventStore<DomainEventPayload>,
    principal: &PrincipalScope,
) -> Vec<EventEnvelope<DomainEventPayload>> {
    store.replay_visible(principal)
}
