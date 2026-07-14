crate::define_data_event_module!(
    NatsSubjectContractsCommand,
    NatsSubjectContractsOperation,
    append_nats_subject_contracts_event,
    "nats_subject_contracts",
    "NatsSubjectContractRecorded",
    "data_eventing.nats_subject_contracts.event_schema",
    crate::DataEventOperation::SchemaRegister,
    ["event_store", "event_outbox", "nats_subject_registry"]
);

crate::define_data_event_artifacts!(
    NatsSubjectContractsService,
    NatsSubjectContractsRepository,
    NatsSubjectContractsEvent,
    NatsSubjectContractsError,
    EVENT_TYPE,
    EVENT_SCHEMA_NAME
);

pub const SUBJECT_CONTRACT_METADATA: &[&str] = &[
    "visibility",
    "fact_provenance",
    "correlation_id",
    "causation_id",
    "authority_contract_version",
];
