use crate::ddd::{CommandEnvelope, DomainResult, EventEnvelope, EventStore, PrincipalScope};
use crate::domain_event_sourcing_projection::{
    append_canon_event, rebuild_canon_projection, replay_visible_canon_events,
};
use crate::event_sourcing_snapshot_projection::{DomainEventPayload, ProjectionSnapshot};

pub fn append_rebuildable_canon_event<T>(
    store: &mut EventStore<DomainEventPayload>,
    command: &CommandEnvelope<T>,
    event_id: impl Into<String>,
    event_name: &'static str,
) -> DomainResult<EventEnvelope<DomainEventPayload>> {
    append_canon_event(store, command, event_id, event_name)
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
