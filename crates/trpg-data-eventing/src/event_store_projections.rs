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
