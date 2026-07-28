
impl ExtensionContract {
    pub const fn new(
        module_name: &'static str,
        event_type: &'static str,
        operation: ExtensionOperation,
        read_models: &'static [&'static str],
        allowed_capabilities: &'static [ExtensionCapability],
    ) -> Self {
        Self {
            module_name,
            event_type,
            operation,
            read_models,
            allowed_capabilities,
            forbidden_capabilities: FORBIDDEN_CAPABILITIES,
            nats_subject: EXTENSION_NATS_SUBJECT,
            metric: EXTENSION_METRIC,
            required_command_fields: EXTENSION_REQUIRED_COMMAND_FIELDS,
            canon_boundary: EXTENSION_CANON_BOUNDARY,
        }
    }

    pub fn uses_current_safe_names(&self) -> bool {
        [
            self.module_name,
            self.event_type,
            self.nats_subject,
            self.metric,
            self.canon_boundary,
        ]
        .into_iter()
        .all(is_current_safe_name)
            && self.read_models.iter().copied().all(is_current_safe_name)
            && self
                .allowed_capabilities
                .iter()
                .map(ExtensionCapability::as_str)
                .all(is_current_safe_name)
            && self
                .forbidden_capabilities
                .iter()
                .map(ExtensionCapability::as_str)
                .all(is_current_safe_name)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtensionExternalContract {
    pub nats_subject: &'static str,
    pub metric: &'static str,
    pub event_type: &'static str,
    pub openfga_relation: &'static str,
    pub opa_policy: &'static str,
}

impl ExtensionExternalContract {
    pub const fn from_contract(contract: ExtensionContract) -> Self {
        Self {
            nats_subject: contract.nats_subject,
            metric: contract.metric,
            event_type: contract.event_type,
            openfga_relation: OPENFGA_RELATION,
            opa_policy: OPA_POLICY,
        }
    }

    pub fn uses_current_safe_names(&self) -> bool {
        [
            self.nats_subject,
            self.metric,
            self.event_type,
            self.openfga_relation,
            self.opa_policy,
        ]
        .into_iter()
        .all(is_current_safe_name)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtensionObservabilityRecord {
    pub tracing_span: &'static str,
    pub metric_name: &'static str,
    pub audit_action: &'static str,
    pub correlation_id: String,
    pub causation_id: String,
}

impl ExtensionObservabilityRecord {
    pub fn from_command<T>(contract: ExtensionContract, command: &CommandEnvelope<T>) -> Self {
        Self {
            tracing_span: TRACING_SPAN,
            metric_name: contract.metric,
            audit_action: contract.module_name,
            correlation_id: command.correlation_id.as_str().to_owned(),
            causation_id: command.causation_id.as_str().to_owned(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtensionExecution {
    pub event: ExtensionEventEnvelope,
    pub external_contract: ExtensionExternalContract,
    pub observability: ExtensionObservabilityRecord,
}

impl ExtensionExecution {
    pub fn from_command<T>(
        contract: ExtensionContract,
        event: ExtensionEventEnvelope,
        command: &CommandEnvelope<T>,
    ) -> Self {
        Self {
            event,
            external_contract: ExtensionExternalContract::from_contract(contract),
            observability: ExtensionObservabilityRecord::from_command(contract, command),
        }
    }
}

pub fn append_extension_event<T>(
    store: &mut ExtensionEventStore,
    authority: &AuthorityContract,
    command: &CommandEnvelope<T>,
    contract: ExtensionContract,
    evidence_path: &'static str,
) -> KernelResult<ExtensionEventEnvelope> {
    if !contract.uses_current_safe_names() {
        return Err(TrpgError::CodingPolicyViolation(
            "extension_sdk_current_safe_name",
        ));
    }

    if contract
        .allowed_capabilities
        .iter()
        .any(ExtensionCapability::is_forbidden)
    {
        return Err(TrpgError::PolicyDenied);
    }

    authority.validate_command(command)?;

    store.append(
        command,
        contract.event_type,
        ExtensionEvent::ContractRecorded(ExtensionEventRecord {
            module_name: contract.module_name,
            operation: contract.operation,
            evidence_path,
            capabilities: contract.allowed_capabilities.to_vec(),
        }),
    )
}

pub fn record_compatibility_report<T>(
    store: &mut ExtensionEventStore,
    authority: &AuthorityContract,
    command: &CommandEnvelope<T>,
    contract: ExtensionContract,
    report: SdkCompatibilityReport,
) -> KernelResult<ExtensionEventEnvelope> {
    if !report.has_required_fields()
        || report.compatibility_result != CompatibilityResult::Compatible
    {
        return Err(TrpgError::PolicyDenied);
    }

    if !contract.uses_current_safe_names() {
        return Err(TrpgError::CodingPolicyViolation(
            "extension_sdk_current_safe_name",
        ));
    }

    authority.validate_command(command)?;
    store.append(
        command,
        contract.event_type,
        ExtensionEvent::CompatibilityChecked(report),
    )
}

pub fn redact_extension_output(visibility: &Visibility, text: &str) -> String {
    if restricted_visibility(visibility.label()) {
        EXTENSION_REDACTED.to_owned()
    } else {
        text.to_owned()
    }
}

pub fn replay_visible_extension_events(
    store: &ExtensionEventStore,
    principal: &PrincipalScope,
) -> Vec<ExtensionEventEnvelope> {
    store.replay_visible(principal)
}

pub fn extension_contracts() -> Vec<ExtensionContract> {
    vec![
        crate::agent_pack_sdk::contract(),
        crate::plugin_sdk::contract(),
        crate::ruleset_pack_sdk::contract(),
        crate::tool_provider_sdk::contract(),
        crate::adr_0008_plugin_boundaries::contract(),
        crate::extension_compatibility_matrix::contract(),
        crate::sdk::contract(),
        contract(),
    ]
}

pub fn contract() -> ExtensionContract {
    ExtensionContract::new(
        "readme",
        "ExtensionSdkReadmeRecorded",
        ExtensionOperation::Readme,
        &["extension_sdk_index", "sdk_compatibility_report"],
        &[ExtensionCapability::ReadProjection],
    )
}

pub fn append_readme_event<T>(
    store: &mut ExtensionEventStore,
    authority: &AuthorityContract,
    command: &CommandEnvelope<T>,
) -> KernelResult<ExtensionEventEnvelope> {
    append_extension_event(store, authority, command, contract(), "extensions/readme")
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

fn restricted_visibility(label: &VisibilityLabel) -> bool {
    label.is_restricted()
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
