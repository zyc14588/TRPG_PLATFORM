crate::define_data_event_module!(
    RagSnapshotCommand,
    RagSnapshotOperation,
    append_rag_snapshot_event,
    "adr_0010_rag_snapshot_rag_snapshot",
    "RagSnapshotDecisionRecorded",
    "data_eventing.adr_0010_rag_snapshot.event_schema",
    crate::DataEventOperation::ArchitectureDecisionRecord,
    ["architecture_decision_index", "rag_snapshot_read_model"]
);
