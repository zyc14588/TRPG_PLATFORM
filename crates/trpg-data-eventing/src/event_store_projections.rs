crate::define_data_event_module!(
    EventStoreProjectionsCommand,
    EventStoreProjectionsOperation,
    append_event_store_projections_event,
    "CODEX-0061-06-DATA-EVENTING-f0442479c9",
    "event_store_projections",
    "EventStoreProjectionRebuilt",
    "data_eventing.event_store_projections.event_schema",
    crate::DataEventOperation::ProjectionRebuild,
    ["projection_view", "replay_cursor"]
);
