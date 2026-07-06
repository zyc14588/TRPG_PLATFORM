crate::define_data_event_module!(
    SnapshotCommand,
    SnapshotOperation,
    append_snapshot_event,
    "CODEX-0616-06-DATA-EVENTING-0b28c2b885",
    "snapshot",
    "SnapshotRecorded",
    "data_eventing.snapshot.event_schema",
    crate::DataEventOperation::SnapshotCreate,
    ["snapshot_store", "event_store"]
);

crate::define_data_event_artifacts!(
    SnapshotService,
    SnapshotRepository,
    SnapshotEvent,
    SnapshotError,
    EVENT_TYPE,
    EVENT_SCHEMA_NAME
);

pub const SNAPSHOT_REBUILD_SOURCE: &str = "event_store";
