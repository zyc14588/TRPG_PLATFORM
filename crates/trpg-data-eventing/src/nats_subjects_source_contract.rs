crate::define_data_event_module!(
    NatsSubjectsSourceContractCommand,
    NatsSubjectsSourceContractOperation,
    append_nats_subjects_source_contract_event,
    "CODEX-0630-06-DATA-EVENTING-fe8798d507",
    "nats_subjects_source_contract",
    "NatsSubjectsSourceContractRecorded",
    "data_eventing.nats_subjects_source_contract.event_schema",
    crate::DataEventOperation::SchemaRegister,
    ["event_store", "event_outbox", "nats_subject_registry"]
);

crate::define_data_event_artifacts!(
    NatsSubjectsSourceContractService,
    NatsSubjectsSourceContractRepository,
    NatsSubjectsSourceContractEvent,
    NatsSubjectsSourceContractError,
    EVENT_TYPE,
    EVENT_SCHEMA_NAME
);

pub const SOURCE_CONTRACT_FIELDS: &[&str] = &[
    "source_event_sequence",
    "visibility",
    "fact_provenance",
    "subject",
    "schema_name",
];
