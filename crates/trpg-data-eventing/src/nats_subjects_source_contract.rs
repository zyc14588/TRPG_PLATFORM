crate::define_data_event_module!(
    NatsSubjectsSourceContractCommand,
    NatsSubjectsSourceContractOperation,
    append_nats_subjects_source_contract_event,
    "nats_subjects_source_contract",
    "NatsSubjectsSourceContractRecorded",
    "data_eventing.nats_subjects_source_contract.event_schema",
    crate::DataEventOperation::SchemaRegister,
    ["event_store", "event_outbox", "nats_subject_registry"]
);

crate::define_data_event_artifacts!(
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
