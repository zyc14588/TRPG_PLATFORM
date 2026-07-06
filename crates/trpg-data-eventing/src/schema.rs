crate::define_data_event_module!(
    SchemaCommand,
    SchemaOperation,
    append_schema_event,
    "CODEX-0609-06-DATA-EVENTING-6f17ea580b",
    "schema",
    "DataEventSchemaRegistered",
    "data_eventing.schema.event_schema",
    crate::DataEventOperation::SchemaRegister,
    ["schema_registry", "event_store"]
);

crate::define_data_event_artifacts!(
    SchemaService,
    SchemaRepository,
    SchemaEvent,
    SchemaError,
    EVENT_TYPE,
    EVENT_SCHEMA_NAME
);

pub fn required_command_fields() -> &'static [&'static str] {
    crate::COMMAND_ENVELOPE_REQUIRED_FIELDS
}

pub fn required_event_fields() -> &'static [&'static str] {
    crate::EVENT_ENVELOPE_REQUIRED_FIELDS
}
