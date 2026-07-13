crate::define_data_event_module!(
    SnapshotCommand,
    SnapshotOperation,
    append_snapshot_event,
    "snapshot",
    "SnapshotRecorded",
    "data_eventing.snapshot.event_schema",
    crate::DataEventOperation::SnapshotCreate,
    ["snapshot_store", "event_store"]
);

crate::define_data_event_artifacts!(SnapshotEvent, SnapshotError, EVENT_TYPE, EVENT_SCHEMA_NAME);

pub const SNAPSHOT_REBUILD_SOURCE: &str = "event_store";
