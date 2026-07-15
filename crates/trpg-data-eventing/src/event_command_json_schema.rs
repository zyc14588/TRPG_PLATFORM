crate::define_data_event_module!(
    EventCommandJsonSchemaCommand,
    EventCommandJsonSchemaOperation,
    append_event_command_json_schema_event,
    "event_command_json_schema",
    "EventCommandJsonSchemaRegistered",
    "data_eventing.event_command_json_schema.event_schema",
    crate::DataEventOperation::SchemaRegister,
    ["schema_registry", "event_store"]
);

crate::define_data_event_artifacts!(
    EventCommandJsonSchemaService,
    EventCommandJsonSchemaRepository,
    EventCommandJsonSchemaEvent,
    EventCommandJsonSchemaError,
    EVENT_TYPE,
    EVENT_SCHEMA_NAME
);

pub fn command_schema_fields() -> &'static [&'static str] {
    crate::COMMAND_ENVELOPE_REQUIRED_FIELDS
}

pub fn event_schema_fields() -> &'static [&'static str] {
    crate::EVENT_ENVELOPE_REQUIRED_FIELDS
}
