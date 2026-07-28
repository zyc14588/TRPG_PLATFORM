
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
