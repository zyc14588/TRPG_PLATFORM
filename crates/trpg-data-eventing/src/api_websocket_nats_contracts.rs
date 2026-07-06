crate::define_data_event_module!(
    ApiWebsocketNatsContractsCommand,
    ApiWebsocketNatsContractsOperation,
    append_api_websocket_nats_contracts_event,
    "CODEX-0626-06-DATA-EVENTING-59249231e5",
    "api_websocket_nats_contracts",
    "ApiWebsocketNatsContractRecorded",
    "data_eventing.api_websocket_nats_contracts.event_schema",
    crate::DataEventOperation::SchemaRegister,
    ["event_store", "event_outbox", "websocket_delivery_contract"]
);

crate::define_data_event_artifacts!(
    ApiWebsocketNatsContractsService,
    ApiWebsocketNatsContractsRepository,
    ApiWebsocketNatsContractsEvent,
    ApiWebsocketNatsContractsError,
    EVENT_TYPE,
    EVENT_SCHEMA_NAME
);

pub const API_CONTRACT_REQUIRED_FIELDS: &[&str] = &[
    "idempotency_key",
    "expected_version",
    "actor",
    "authority_mode",
    "visibility",
    "fact_provenance",
    "correlation_id",
    "causation_id",
];

pub const DELIVERY_SUBJECTS: &[&str] = &[
    crate::NATS_EVENTS_APPENDED,
    crate::NATS_PROJECTION_REBUILD_REQUESTED,
];
