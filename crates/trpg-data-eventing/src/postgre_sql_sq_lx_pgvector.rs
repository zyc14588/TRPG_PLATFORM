crate::define_data_event_module!(
    PostgreSqlSqLxPgvectorCommand,
    PostgreSqlSqLxPgvectorOperation,
    append_postgre_sql_sq_lx_pgvector_event,
    "postgre_sql_sq_lx_pgvector",
    "PostgreSqlPgvectorIndexed",
    "data_eventing.postgre_sql_sq_lx_pgvector.event_schema",
    crate::DataEventOperation::ProjectionRebuild,
    ["rag_index", "event_store", "snapshot_store"]
);

crate::define_data_event_artifacts!(
    PostgreSqlSqLxPgvectorEvent,
    PostgreSqlSqLxPgvectorError,
    EVENT_TYPE,
    EVENT_SCHEMA_NAME
);

pub const RAG_INDEX_FIELDS: &[&str] = &["source_event_sequence", "visibility", "fact_provenance"];
