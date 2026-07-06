crate::define_data_event_module!(
    ReadmeCommand,
    ReadmeOperation,
    append_readme_event,
    "CODEX-0615-06-DATA-EVENTING-58af1867fc",
    "readme",
    "DataEventingReadmeTraceRecorded",
    "data_eventing.readme.event_schema",
    crate::DataEventOperation::ArchitectureDecisionRecord,
    ["traceability_matrix"]
);

crate::define_data_event_artifacts!(
    ReadmeService,
    ReadmeRepository,
    ReadmeEvent,
    ReadmeError,
    EVENT_TYPE,
    EVENT_SCHEMA_NAME
);

pub const README_GOVERNANCE_BOUNDARY: &str =
    "documents_trace_event_store_visibility_fact_provenance";
