crate::define_data_event_module!(
    EventSourcingSnapshotProjectionCommand,
    EventSourcingSnapshotProjectionOperation,
    append_event_sourcing_snapshot_projection_event,
    "event_sourcing_snapshot_projection",
    "EventSourcingSnapshotProjectionRecorded",
    "data_eventing.event_sourcing_snapshot_projection.event_schema",
    crate::DataEventOperation::ProjectionRebuild,
    ["event_store", "snapshot_store", "projection_checkpoint"]
);

crate::define_data_event_artifacts!(
    EventSourcingSnapshotProjectionService,
    EventSourcingSnapshotProjectionRepository,
    EventSourcingSnapshotProjectionEvent,
    EventSourcingSnapshotProjectionError,
    EVENT_TYPE,
    EVENT_SCHEMA_NAME
);

pub const SNAPSHOT_BOUNDARY: &str = "event_store_is_canon_projection_is_rebuildable";
