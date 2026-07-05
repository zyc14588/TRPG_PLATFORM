crate::define_data_event_module!(
    RagSnapshotCommand,
    RagSnapshotOperation,
    append_rag_snapshot_event,
    "CODEX-0588-06-DATA-EVENTING-ca435ac678",
    "adr_0010_rag_snapshot_rag_snapshot",
    "RagSnapshotDecisionRecorded",
    "data_eventing.adr_0010_rag_snapshot.event_schema",
    crate::DataEventOperation::ArchitectureDecisionRecord,
    ["architecture_decision_index", "rag_snapshot_read_model"]
);
