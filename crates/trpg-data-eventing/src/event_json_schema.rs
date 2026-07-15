crate::define_data_event_module!(
    EventJsonSchemaCommand,
    EventJsonSchemaOperation,
    append_event_json_schema_event,
    "event_json_schema",
    "EventJsonSchemaRegistered",
    "data_eventing.event_json_schema.event_schema",
    crate::DataEventOperation::SchemaRegister,
    ["schema_registry", "event_store", "event_outbox"]
);

crate::define_data_event_artifacts!(
    EventJsonSchemaService,
    EventJsonSchemaRepository,
    EventJsonSchemaEvent,
    EventJsonSchemaError,
    EVENT_TYPE,
    EVENT_SCHEMA_NAME
);

pub const SCHEMA_KIND: &str = "event_command_schema_aggregate";
pub const CANONICAL_WRITE_FLOW: &str = "command_workflow_decision_event_store_projection";
pub const READ_MODEL_POLICY: &str = "projection_cache_rag_rebuildable";

pub const GOVERNANCE_ERROR_CODES: &[&str] = &[
    "AUTHORITY_VIOLATION",
    "AUTHORITY_CONTRACT_MUTATION",
    "DIRECT_AGENT_STATE_WRITE",
    "POLICY_DENIED",
    "EXPECTED_VERSION_CONFLICT",
    "DUPLICATE_COMMAND",
    "VISIBILITY_DENIED",
];

pub const VISIBILITY_LABELS: &[&str] = &[
    "public",
    "party_visible",
    "private_to_player",
    "keeper_only",
    "investigator_private",
    "ai_internal",
    "system_only",
    "system_private",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventJsonSchemaCatalogEntry {
    pub schema_name: &'static str,
    pub schema_kind: &'static str,
    pub command_fields: &'static [&'static str],
    pub event_fields: &'static [&'static str],
    pub visibility_labels: &'static [&'static str],
    pub governance_error_codes: &'static [&'static str],
    pub nats_subjects: &'static [&'static str],
    pub metrics: &'static [&'static str],
    pub canonical_write_flow: &'static str,
    pub read_model_policy: &'static str,
}

pub fn catalog_entry() -> EventJsonSchemaCatalogEntry {
    EventJsonSchemaCatalogEntry {
        schema_name: EVENT_SCHEMA_NAME,
        schema_kind: SCHEMA_KIND,
        command_fields: crate::COMMAND_ENVELOPE_REQUIRED_FIELDS,
        event_fields: crate::EVENT_ENVELOPE_REQUIRED_FIELDS,
        visibility_labels: VISIBILITY_LABELS,
        governance_error_codes: GOVERNANCE_ERROR_CODES,
        nats_subjects: crate::DATA_EVENT_NATS_SUBJECTS,
        metrics: crate::DATA_EVENT_METRICS,
        canonical_write_flow: CANONICAL_WRITE_FLOW,
        read_model_policy: READ_MODEL_POLICY,
    }
}

pub fn command_schema_fields() -> &'static [&'static str] {
    crate::COMMAND_ENVELOPE_REQUIRED_FIELDS
}

pub fn event_schema_fields() -> &'static [&'static str] {
    crate::EVENT_ENVELOPE_REQUIRED_FIELDS
}
