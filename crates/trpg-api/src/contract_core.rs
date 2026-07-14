use trpg_contracts::{canonical_event_registry, EventDescriptor};
use trpg_shared_kernel::{
    AuthorityContract, CommandEnvelope, EventEnvelope, EventStore, KernelResult, PrincipalScope,
    TrpgError, VisibilityLabel,
};

pub const COMMAND_ENDPOINT: &str = "/api/v1/transport/commands";
pub const COMMAND_DISPATCHED_SUBJECT: &str = "trpg.api.command.dispatched";
pub const REALTIME_DELTA_SUBJECT: &str = "trpg.realtime.delta.broadcast";
pub const CANONICAL_WRITE_FLOW: &str = "command_workflow_decision_event_store_projection";
pub const HTTP_FRAMEWORK: &str = "axum";
pub const OPENAPI_GENERATOR: &str = "utoipa";
pub const COMMAND_HANDLER_SYMBOL: &str = "handle_transport_command";
pub const WEBSOCKET_SYNC_ENDPOINT: &str = "/ws/v1/campaigns/{campaign_id}/rooms/{room_id}";
pub const SQLX_EVENT_STORE_ADAPTER_BOUNDARY: &str = "sqlx_event_store_transaction_boundary";
pub const FORMAL_STATE_WRITE_BOUNDARY: &str = "state_service_event_store_boundary";
pub const HTTP_ACTION_METHOD: &str = "POST";
pub const HTTP_ACTION_ENDPOINT: &str = "/campaigns/{id}/actions";

pub const REQUIRED_HTTP_HEADERS: &[&str] = &[
    "idempotency_key",
    "expected_version",
    "correlation_id",
    "causation_id",
    "authority_contract_version",
];

pub const POLICY_GATES: &[&str] = &[
    "openfga_relationship_check",
    "opa_context_policy",
    "tool_permission_gate_default_deny",
];

pub const TOOL_PERMISSION_CHECKS: &[&str] = &[
    "authority_contract",
    "agent_permission_profile",
    "campaign_state",
    "ruleset_compatibility",
    "visibility",
    "fact_provenance",
    "tool_schema_version",
    "safety",
];

pub const ADAPTER_BOUNDARIES: &[&str] = &[
    "axum_handler_boundary",
    "utoipa_schema_boundary",
    SQLX_EVENT_STORE_ADAPTER_BOUNDARY,
    "websocket_room_sync_boundary",
    "nats_outbox_publish_boundary",
];

pub const REQUIRED_COMMAND_FIELDS: &[&str] = &[
    "command_id",
    "idempotency_key",
    "expected_version",
    "actor",
    "authority_mode",
    "authority_contract_version",
    "visibility",
    "fact_provenance",
    "correlation_id",
    "causation_id",
    "write_path",
];

pub const REQUIRED_EVENT_FIELDS: &[&str] = &[
    "sequence",
    "event_type",
    "command_id",
    "idempotency_key",
    "authority_contract_version",
    "visibility",
    "fact_provenance",
    "correlation_id",
    "causation_id",
    "payload",
];

pub const API_REALTIME_METRICS: &[&str] = &[
    "trpg_command_total",
    "trpg_event_append_latency_ms",
    "trpg_policy_deny_total",
    "trpg_projection_lag_events",
    "trpg_visibility_redaction_total",
];

pub const OBSERVABILITY_FIELDS: &[&str] = &[
    "correlation_id",
    "causation_id",
    "visibility",
    "fact_provenance",
    "authority_contract_version",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApiRealtimeOperation {
    ValidateTransportCommand,
    DispatchCommand,
    PublishRealtimeDelta,
    RegisterSchema,
    RegisterProviderContract,
}

impl ApiRealtimeOperation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ValidateTransportCommand => "validate_transport_command",
            Self::DispatchCommand => "dispatch_command",
            Self::PublishRealtimeDelta => "publish_realtime_delta",
            Self::RegisterSchema => "register_schema",
            Self::RegisterProviderContract => "register_provider_contract",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ApiRealtimeContract {
    pub module_name: &'static str,
    pub endpoint: &'static str,
    pub event_type: &'static str,
    pub event_schema_name: &'static str,
    pub operation: ApiRealtimeOperation,
    pub nats_subjects: &'static [&'static str],
    pub required_command_fields: &'static [&'static str],
    pub required_event_fields: &'static [&'static str],
    pub metrics: &'static [&'static str],
    pub observability_fields: &'static [&'static str],
    pub canonical_write_flow: &'static str,
}

impl ApiRealtimeContract {
    pub const fn new(
        module_name: &'static str,
        event_type: &'static str,
        event_schema_name: &'static str,
        operation: ApiRealtimeOperation,
    ) -> Self {
        Self {
            module_name,
            endpoint: COMMAND_ENDPOINT,
            event_type,
            event_schema_name,
            operation,
            nats_subjects: &[COMMAND_DISPATCHED_SUBJECT, REALTIME_DELTA_SUBJECT],
            required_command_fields: REQUIRED_COMMAND_FIELDS,
            required_event_fields: REQUIRED_EVENT_FIELDS,
            metrics: API_REALTIME_METRICS,
            observability_fields: OBSERVABILITY_FIELDS,
            canonical_write_flow: CANONICAL_WRITE_FLOW,
        }
    }

    pub fn uses_current_safe_names(&self) -> bool {
        [
            self.module_name,
            self.event_type,
            self.event_schema_name,
            self.operation.as_str(),
            self.canonical_write_flow,
        ]
        .iter()
        .all(|value| is_current_safe_name(value))
            && self
                .nats_subjects
                .iter()
                .all(|value| is_current_safe_name(value))
            && self.metrics.iter().all(|value| is_current_safe_name(value))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiCommandPayload {
    pub module_name: &'static str,
    pub operation: ApiRealtimeOperation,
    pub request_summary: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiRealtimeEventPayload {
    pub module_name: &'static str,
    pub operation: ApiRealtimeOperation,
    pub endpoint: &'static str,
    pub event_schema_name: &'static str,
    pub nats_subjects: &'static [&'static str],
    pub observability_fields: &'static [&'static str],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RealtimeDelta {
    pub sequence: u64,
    pub event_type: &'static str,
    pub module_name: &'static str,
    pub visibility_label: &'static str,
    pub correlation_id: String,
    pub causation_id: String,
    pub provenance_reference: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiProjection {
    pub event_count: usize,
    pub last_sequence: u64,
    pub modules: Vec<&'static str>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpenApiContractDocument {
    pub command_endpoint: &'static str,
    pub framework: &'static str,
    pub generator: &'static str,
    pub required_headers: &'static [&'static str],
    pub schemas: Vec<&'static str>,
    pub websocket_delta_subject: &'static str,
    pub policy_gates: &'static [&'static str],
    pub adapter_boundaries: &'static [&'static str],
    pub event_registry: &'static [EventDescriptor],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderAccessPath {
    AgentGateway,
    AgentRuntimeAdapter,
    DirectModelProvider,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderPolicyDecision {
    pub allowed: bool,
    pub route: ProviderAccessPath,
    pub audit_fields: &'static [&'static str],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HttpApiAdapterContract {
    pub framework: &'static str,
    pub route: &'static str,
    pub method: &'static str,
    pub handler_symbol: &'static str,
    pub openapi_generator: &'static str,
    pub dto_schema: &'static str,
    pub required_headers: &'static [&'static str],
    pub policy_gates: &'static [&'static str],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RealtimeAdapterContract {
    pub websocket_endpoint: &'static str,
    pub nats_subjects: &'static [&'static str],
    pub replayable_events: &'static [EventDescriptor],
    pub visibility_filtered: bool,
    pub reconnect_supported: bool,
    pub multi_room_supported: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PersistenceAdapterContract {
    pub event_store_table: &'static str,
    pub adapter_boundary: &'static str,
    pub transaction_boundary: &'static str,
    pub formal_state_write_boundary: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ToolPermissionGateContract {
    pub default_allow: bool,
    pub checks: &'static [&'static str],
    pub policy_gates: &'static [&'static str],
    pub formal_state_tools_require_agent_gateway: bool,
}

pub fn validate_api_contract(api_contract: &ApiRealtimeContract) -> KernelResult<()> {
    if api_contract.endpoint != COMMAND_ENDPOINT {
        return Err(TrpgError::InvalidConfiguration("api_endpoint"));
    }
    if !api_contract.uses_current_safe_names() {
        return Err(TrpgError::CodingPolicyViolation("api_current_safe_name"));
    }
    for field in REQUIRED_COMMAND_FIELDS {
        if !api_contract.required_command_fields.contains(field) {
            return Err(TrpgError::InvalidConfiguration("api_command_field"));
        }
    }
    for field in REQUIRED_EVENT_FIELDS {
        if !api_contract.required_event_fields.contains(field) {
            return Err(TrpgError::InvalidConfiguration("api_event_field"));
        }
    }
    for subject in api_contract.nats_subjects {
        validate_nats_subject(subject)?;
    }
    Ok(())
}

pub fn append_api_contract_event<T>(
    store: &mut EventStore<ApiRealtimeEventPayload>,
    authority: &AuthorityContract,
    command: &CommandEnvelope<T>,
    api_contract: &ApiRealtimeContract,
) -> KernelResult<EventEnvelope<ApiRealtimeEventPayload>> {
    validate_api_contract(api_contract)?;
    authority.validate_command(command)?;
    store.append(
        command,
        api_contract.event_type,
        ApiRealtimeEventPayload {
            module_name: api_contract.module_name,
            operation: api_contract.operation,
            endpoint: api_contract.endpoint,
            event_schema_name: api_contract.event_schema_name,
            nats_subjects: api_contract.nats_subjects,
            observability_fields: api_contract.observability_fields,
        },
    )
}

pub fn visible_realtime_delta(
    event: &EventEnvelope<ApiRealtimeEventPayload>,
    principal: &PrincipalScope,
) -> Option<RealtimeDelta> {
    if !event.visibility.can_view(principal) {
        return None;
    }

    Some(RealtimeDelta {
        sequence: event.sequence,
        event_type: event.event_type,
        module_name: event.payload.module_name,
        visibility_label: event.visibility.label().as_str(),
        correlation_id: event.correlation_id.as_str().to_owned(),
        causation_id: event.causation_id.as_str().to_owned(),
        provenance_reference: event.fact_provenance.reference.as_str().to_owned(),
    })
}

pub fn replay_visible_deltas(
    store: &EventStore<ApiRealtimeEventPayload>,
    principal: &PrincipalScope,
) -> Vec<RealtimeDelta> {
    store
        .events()
        .iter()
        .filter_map(|event| visible_realtime_delta(event, principal))
        .collect()
}

pub fn rebuild_api_projection(events: &[EventEnvelope<ApiRealtimeEventPayload>]) -> ApiProjection {
    let mut modules = Vec::new();
    for event in events {
        if !modules.contains(&event.payload.module_name) {
            modules.push(event.payload.module_name);
        }
    }

    ApiProjection {
        event_count: events.len(),
        last_sequence: events.last().map(|event| event.sequence).unwrap_or(0),
        modules,
    }
}

pub fn build_openapi_contract_document(
    contracts: &[ApiRealtimeContract],
) -> OpenApiContractDocument {
    let mut schemas = Vec::new();
    for api_contract in contracts {
        if !schemas.contains(&api_contract.event_schema_name) {
            schemas.push(api_contract.event_schema_name);
        }
    }

    OpenApiContractDocument {
        command_endpoint: COMMAND_ENDPOINT,
        framework: HTTP_FRAMEWORK,
        generator: OPENAPI_GENERATOR,
        required_headers: REQUIRED_HTTP_HEADERS,
        schemas,
        websocket_delta_subject: REALTIME_DELTA_SUBJECT,
        policy_gates: POLICY_GATES,
        adapter_boundaries: ADAPTER_BOUNDARIES,
        event_registry: trpg_contracts::canonical_event_registry(),
    }
}

pub fn validate_nats_subject(subject: &str) -> KernelResult<()> {
    if !subject.starts_with("trpg.") || subject.contains('*') || subject.contains('>') {
        return Err(TrpgError::InvalidConfiguration("nats_subject"));
    }
    if !is_current_safe_name(subject) {
        return Err(TrpgError::CodingPolicyViolation("nats_current_safe_name"));
    }
    Ok(())
}

pub fn validate_domain_nats_subject(subject: &str) -> KernelResult<()> {
    if !(subject.starts_with("trpg.") || subject.starts_with("campaign."))
        || subject.contains('*')
        || subject.contains('>')
    {
        return Err(TrpgError::InvalidConfiguration("domain_nats_subject"));
    }
    if !is_current_safe_name(subject) {
        return Err(TrpgError::CodingPolicyViolation(
            "domain_nats_current_safe_name",
        ));
    }
    Ok(())
}

pub fn evaluate_provider_access(route: ProviderAccessPath) -> KernelResult<ProviderPolicyDecision> {
    match route {
        ProviderAccessPath::AgentGateway => Ok(ProviderPolicyDecision {
            allowed: true,
            route,
            audit_fields: OBSERVABILITY_FIELDS,
        }),
        ProviderAccessPath::AgentRuntimeAdapter | ProviderAccessPath::DirectModelProvider => {
            Err(TrpgError::PolicyDenied)
        }
    }
}

pub fn http_api_adapter_contract() -> HttpApiAdapterContract {
    HttpApiAdapterContract {
        framework: HTTP_FRAMEWORK,
        route: HTTP_ACTION_ENDPOINT,
        method: HTTP_ACTION_METHOD,
        handler_symbol: COMMAND_HANDLER_SYMBOL,
        openapi_generator: OPENAPI_GENERATOR,
        dto_schema: "ApiCommandPayload",
        required_headers: REQUIRED_HTTP_HEADERS,
        policy_gates: POLICY_GATES,
    }
}

pub fn realtime_adapter_contract() -> RealtimeAdapterContract {
    RealtimeAdapterContract {
        websocket_endpoint: WEBSOCKET_SYNC_ENDPOINT,
        nats_subjects: &[COMMAND_DISPATCHED_SUBJECT, REALTIME_DELTA_SUBJECT],
        replayable_events: canonical_event_registry(),
        visibility_filtered: true,
        reconnect_supported: true,
        multi_room_supported: true,
    }
}

pub fn persistence_adapter_contract() -> PersistenceAdapterContract {
    PersistenceAdapterContract {
        event_store_table: "event_store",
        adapter_boundary: SQLX_EVENT_STORE_ADAPTER_BOUNDARY,
        transaction_boundary: "command_workflow_event_store_transaction",
        formal_state_write_boundary: FORMAL_STATE_WRITE_BOUNDARY,
    }
}

pub fn tool_permission_gate_contract() -> ToolPermissionGateContract {
    ToolPermissionGateContract {
        default_allow: false,
        checks: TOOL_PERMISSION_CHECKS,
        policy_gates: POLICY_GATES,
        formal_state_tools_require_agent_gateway: true,
    }
}

pub fn validate_primary_adapter_boundaries() -> KernelResult<()> {
    let http = http_api_adapter_contract();
    if http.framework != HTTP_FRAMEWORK || http.openapi_generator != OPENAPI_GENERATOR {
        return Err(TrpgError::InvalidConfiguration("http_openapi_adapter"));
    }
    for header in REQUIRED_HTTP_HEADERS {
        if !http.required_headers.contains(header) {
            return Err(TrpgError::InvalidConfiguration("http_required_header"));
        }
    }
    for gate in POLICY_GATES {
        if !http.policy_gates.contains(gate) {
            return Err(TrpgError::InvalidConfiguration("policy_gate"));
        }
    }

    let realtime = realtime_adapter_contract();
    for subject in realtime.nats_subjects {
        validate_domain_nats_subject(subject)?;
    }
    if !realtime.visibility_filtered
        || !realtime.reconnect_supported
        || !realtime.multi_room_supported
    {
        return Err(TrpgError::InvalidConfiguration("realtime_adapter"));
    }

    let persistence = persistence_adapter_contract();
    if persistence.adapter_boundary != SQLX_EVENT_STORE_ADAPTER_BOUNDARY
        || persistence.formal_state_write_boundary != FORMAL_STATE_WRITE_BOUNDARY
    {
        return Err(TrpgError::InvalidConfiguration("persistence_adapter"));
    }

    let tool_gate = tool_permission_gate_contract();
    if tool_gate.default_allow || !tool_gate.formal_state_tools_require_agent_gateway {
        return Err(TrpgError::PolicyDenied);
    }
    for required_check in TOOL_PERMISSION_CHECKS {
        if !tool_gate.checks.contains(required_check) {
            return Err(TrpgError::InvalidConfiguration("tool_permission_check"));
        }
    }

    evaluate_provider_access(ProviderAccessPath::AgentGateway)?;
    if evaluate_provider_access(ProviderAccessPath::AgentRuntimeAdapter).is_ok()
        || evaluate_provider_access(ProviderAccessPath::DirectModelProvider).is_ok()
    {
        return Err(TrpgError::PolicyDenied);
    }

    Ok(())
}

pub fn event_visibility_label(event: &EventEnvelope<ApiRealtimeEventPayload>) -> &VisibilityLabel {
    event.visibility.label()
}

pub fn is_current_safe_name(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return false;
    }

    let lower = trimmed.to_ascii_lowercase();
    let denied = [
        "generated-from-source",
        "generated_from_source",
        "source-breakdow",
        "source_breakdow",
        "docs-implementation",
        "docs_implementation",
        "implementation-90",
        "implementation_90",
        "fix-history",
        "fix_history",
        "legacy",
        "v3",
        "v4",
        "v5",
        "v6",
    ];

    if denied.iter().any(|token| lower.contains(token)) {
        return false;
    }

    trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
        && !has_long_hex_run(trimmed)
}

fn has_long_hex_run(value: &str) -> bool {
    let mut run = 0;
    for ch in value.chars() {
        if ch.is_ascii_hexdigit() {
            run += 1;
            if run >= 10 {
                return true;
            }
        } else {
            run = 0;
        }
    }
    false
}

#[macro_export]
macro_rules! define_api_realtime_contract_module {
    (
        $module_name:literal,
        $event_type:literal,
        $event_schema_name:literal,
        $operation:expr
    ) => {
        pub const MODULE_NAME: &str = $module_name;
        pub const EVENT_TYPE: &str = $event_type;
        pub const EVENT_SCHEMA_NAME: &str = $event_schema_name;

        pub fn contract() -> $crate::contract_core::ApiRealtimeContract {
            $crate::contract_core::ApiRealtimeContract::new(
                MODULE_NAME,
                EVENT_TYPE,
                EVENT_SCHEMA_NAME,
                $operation,
            )
        }

        pub fn append_contract_event<T>(
            store: &mut trpg_shared_kernel::EventStore<
                $crate::contract_core::ApiRealtimeEventPayload,
            >,
            authority: &trpg_shared_kernel::AuthorityContract,
            command: &trpg_shared_kernel::CommandEnvelope<T>,
        ) -> trpg_shared_kernel::KernelResult<
            trpg_shared_kernel::EventEnvelope<$crate::contract_core::ApiRealtimeEventPayload>,
        > {
            $crate::contract_core::append_api_contract_event(store, authority, command, &contract())
        }
    };
}
