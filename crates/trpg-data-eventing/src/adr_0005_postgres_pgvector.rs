crate::define_data_event_module!(
    Adr0005PostgresPgvectorCommand,
    Adr0005PostgresPgvectorOperation,
    append_adr_0005_postgres_pgvector_event,
    "adr_0005_postgres_pgvector",
    "Adr0005PostgresPgvectorDecisionRecorded",
    "data_eventing.adr_0005_postgres_pgvector.event_schema",
    crate::DataEventOperation::ArchitectureDecisionRecord,
    ["event_store", "rag_index", "snapshot_store"]
);
