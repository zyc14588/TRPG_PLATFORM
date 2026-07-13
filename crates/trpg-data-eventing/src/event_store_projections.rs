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
