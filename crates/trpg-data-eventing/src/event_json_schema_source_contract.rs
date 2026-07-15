crate::define_data_event_module!(
    EventJsonSchemaSourceContractCommand,
    EventJsonSchemaSourceContractOperation,
    append_event_json_schema_source_contract_event,
    "event_json_schema_source_contract",
    "EventJsonSchemaSourceContractRecorded",
    "data_eventing.event_json_schema_source_contract.event_schema",
    crate::DataEventOperation::SchemaRegister,
    ["event_schema_catalog", "schema_source_contract"]
);
