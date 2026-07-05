crate::define_data_event_module!(
    PostgresPgvectorCommand,
    PostgresPgvectorOperation,
    append_postgres_pgvector_event,
    "CODEX-0587-06-DATA-EVENTING-157ac308e6",
    "adr_0005_postgres_pgvector_postgre_sql_pgvector",
    "PostgresPgvectorDecisionRecorded",
    "data_eventing.adr_0005_postgres_pgvector.event_schema",
    crate::DataEventOperation::ArchitectureDecisionRecord,
    ["architecture_decision_index", "vector_index_read_model"]
);
