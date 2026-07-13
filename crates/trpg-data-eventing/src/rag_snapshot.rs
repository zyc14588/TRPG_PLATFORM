crate::define_data_event_module!(
    RagSnapshotCommand,
    RagSnapshotOperation,
    append_rag_snapshot_event,
    "rag_snapshot",
    "RagSnapshotRecorded",
    "data_eventing.rag_snapshot.event_schema",
    crate::DataEventOperation::SnapshotCreate,
    ["event_store", "rag_snapshot_store", "rag_index"]
);

crate::define_data_event_artifacts!(
    RagSnapshotEvent,
    RagSnapshotError,
    EVENT_TYPE,
    EVENT_SCHEMA_NAME
);

pub const RAG_SNAPSHOT_METADATA_FIELDS: &[&str] = &[
    "source_type",
    "visibility",
    "version",
    "owner",
    "allowed_use",
    "fact_provenance",
    "source_event_sequence",
    "chunk_hash",
];

pub const RAG_REBUILD_SOURCE: &str = crate::EVENT_STORE_TABLE;
