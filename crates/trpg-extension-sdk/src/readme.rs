use std::error::Error;
use std::fmt;

use trpg_shared_kernel::{
    AuthorityContract, CommandEnvelope, EventEnvelope, EventStore, KernelResult, PrincipalScope,
    TrpgError, Visibility, VisibilityLabel, WireErrorCode,
};

pub const EXTENSION_REDACTED: &str = "[redacted]";
pub const EXTENSION_CANON_BOUNDARY: &str = "command_workflow_decision_event_store_projection";
pub const EXTENSION_NATS_SUBJECT: &str = "trpg.extension_sdk.contract.recorded";
pub const EXTENSION_METRIC: &str = "trpg_extension_sdk_contract_total";
pub const OPENFGA_RELATION: &str = "extension_sdk_operator";
pub const OPA_POLICY: &str = "extension_sdk_policy";
pub const TRACING_SPAN: &str = "extension_sdk.contract.record";

pub const EXTENSION_REQUIRED_COMMAND_FIELDS: &[&str] = &[
    "idempotency_key",
    "expected_version",
    "actor",
    "authority_mode",
    "visibility",
    "fact_provenance",
    "correlation_id",
    "causation_id",
];

pub const FORBIDDEN_CAPABILITIES: &[ExtensionCapability] = &[
    ExtensionCapability::AppendEventStore,
    ExtensionCapability::DirectLlm,
    ExtensionCapability::DatabaseWrite,
    ExtensionCapability::InternalToolGateAccess,
    ExtensionCapability::ModifyAuthorityContract,
    ExtensionCapability::ForgeDice,
    ExtensionCapability::RevealRestrictedVisibility,
];

pub type ExtensionSdkResult<T> = Result<T, ExtensionSdkError>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExtensionSdkError {
    ForbiddenCapability(WireErrorCode),
    CompatibilityRejected(WireErrorCode),
    Kernel(TrpgError),
}

impl ExtensionSdkError {
    pub fn wire_code(&self) -> WireErrorCode {
        match self {
            Self::ForbiddenCapability(code) | Self::CompatibilityRejected(code) => *code,
            Self::Kernel(error) => error.wire_code(),
        }
    }

    pub fn code(&self) -> &'static str {
        self.wire_code().as_str()
    }
}

impl From<TrpgError> for ExtensionSdkError {
    fn from(value: TrpgError) -> Self {
        Self::Kernel(value)
    }
}

impl fmt::Display for ExtensionSdkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.code())
    }
}

impl Error for ExtensionSdkError {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtensionCapability {
    RegisterPlugin,
    RegisterAgentPack,
    RegisterRulesetPack,
    RegisterToolProvider,
    InvokeGrantedTool,
    ReadProjection,
    EmitProposedDecision,
    AppendEventStore,
    DirectLlm,
    DatabaseWrite,
    InternalToolGateAccess,
    ModifyAuthorityContract,
    ForgeDice,
    RevealRestrictedVisibility,
}

impl ExtensionCapability {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RegisterPlugin => "register_plugin",
            Self::RegisterAgentPack => "register_agent_pack",
            Self::RegisterRulesetPack => "register_ruleset_pack",
            Self::RegisterToolProvider => "register_tool_provider",
            Self::InvokeGrantedTool => "invoke_granted_tool",
            Self::ReadProjection => "read_projection",
            Self::EmitProposedDecision => "emit_proposed_decision",
            Self::AppendEventStore => "append_event_store",
            Self::DirectLlm => "direct_llm",
            Self::DatabaseWrite => "database_write",
            Self::InternalToolGateAccess => "internal_tool_gate_access",
            Self::ModifyAuthorityContract => "modify_authority_contract",
            Self::ForgeDice => "forge_dice",
            Self::RevealRestrictedVisibility => "reveal_restricted_visibility",
        }
    }

    pub fn is_forbidden(&self) -> bool {
        FORBIDDEN_CAPABILITIES.contains(self)
    }

    pub const fn denial_code(&self) -> WireErrorCode {
        match self {
            Self::AppendEventStore => WireErrorCode::ExtensionStateWriteForbidden,
            Self::DirectLlm => WireErrorCode::ExtensionDirectLlmForbidden,
            Self::DatabaseWrite => WireErrorCode::ExtensionDatabaseWriteForbidden,
            Self::InternalToolGateAccess => WireErrorCode::ExtensionToolGateBypassForbidden,
            Self::ModifyAuthorityContract => WireErrorCode::ExtensionAuthorityContractForbidden,
            Self::ForgeDice => WireErrorCode::ExtensionDiceForgeForbidden,
            Self::RevealRestrictedVisibility => WireErrorCode::ExtensionVisibilityLeakForbidden,
            _ => WireErrorCode::ExtensionCapabilityDenied,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ExtensionCapabilityGrantSet {
    granted: Vec<ExtensionCapability>,
}

impl ExtensionCapabilityGrantSet {
    pub fn with_grants(grants: &[ExtensionCapability]) -> ExtensionSdkResult<Self> {
        let mut grant_set = Self::default();
        for grant in grants {
            grant_set.grant(*grant)?;
        }
        Ok(grant_set)
    }

    pub fn grant(&mut self, capability: ExtensionCapability) -> ExtensionSdkResult<()> {
        if capability.is_forbidden() {
            return Err(ExtensionSdkError::ForbiddenCapability(
                capability.denial_code(),
            ));
        }

        if !self.granted.contains(&capability) {
            self.granted.push(capability);
        }

        Ok(())
    }

    pub fn allows(&self, capability: ExtensionCapability) -> bool {
        !capability.is_forbidden() && self.granted.contains(&capability)
    }

    pub fn require(&self, capability: ExtensionCapability) -> ExtensionSdkResult<()> {
        if self.allows(capability) {
            Ok(())
        } else if capability.is_forbidden() {
            Err(ExtensionSdkError::ForbiddenCapability(
                capability.denial_code(),
            ))
        } else {
            Err(ExtensionSdkError::ForbiddenCapability(
                WireErrorCode::ExtensionCapabilityDenied,
            ))
        }
    }

    pub fn granted(&self) -> &[ExtensionCapability] {
        &self.granted
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtensionPolicyGate {
    pub capability_grants: ExtensionCapabilityGrantSet,
    pub requested_capabilities: Vec<ExtensionCapability>,
    pub tool_grant_allowed: bool,
    pub openfga_allowed: bool,
    pub opa_allowed: bool,
    pub audit_recorded: bool,
}

impl ExtensionPolicyGate {
    pub fn allow(requested_capabilities: &[ExtensionCapability]) -> Self {
        Self {
            capability_grants: ExtensionCapabilityGrantSet::with_grants(requested_capabilities)
                .expect("B044 fixture capabilities are grantable"),
            requested_capabilities: requested_capabilities.to_vec(),
            tool_grant_allowed: true,
            openfga_allowed: true,
            opa_allowed: true,
            audit_recorded: true,
        }
    }

    pub fn default_deny(requested_capabilities: &[ExtensionCapability]) -> Self {
        Self {
            capability_grants: ExtensionCapabilityGrantSet::default(),
            requested_capabilities: requested_capabilities.to_vec(),
            tool_grant_allowed: true,
            openfga_allowed: true,
            opa_allowed: true,
            audit_recorded: true,
        }
    }

    pub fn deny_tool_grant(requested_capabilities: &[ExtensionCapability]) -> Self {
        Self {
            tool_grant_allowed: false,
            ..Self::allow(requested_capabilities)
        }
    }

    pub fn deny_openfga(requested_capabilities: &[ExtensionCapability]) -> Self {
        Self {
            openfga_allowed: false,
            ..Self::allow(requested_capabilities)
        }
    }

    pub fn deny_opa(requested_capabilities: &[ExtensionCapability]) -> Self {
        Self {
            opa_allowed: false,
            ..Self::allow(requested_capabilities)
        }
    }

    pub fn without_audit(requested_capabilities: &[ExtensionCapability]) -> Self {
        Self {
            audit_recorded: false,
            ..Self::allow(requested_capabilities)
        }
    }

    pub fn authorize(&self) -> ExtensionSdkResult<()> {
        if !self.tool_grant_allowed {
            return Err(ExtensionSdkError::ForbiddenCapability(
                WireErrorCode::ExtensionToolGrantDenied,
            ));
        }
        if !self.openfga_allowed {
            return Err(ExtensionSdkError::ForbiddenCapability(
                WireErrorCode::ExtensionOpenFgaDenied,
            ));
        }
        if !self.opa_allowed {
            return Err(ExtensionSdkError::ForbiddenCapability(
                WireErrorCode::ExtensionOpaDenied,
            ));
        }
        if !self.audit_recorded {
            return Err(ExtensionSdkError::ForbiddenCapability(
                WireErrorCode::ExtensionAuditRequired,
            ));
        }
        for capability in &self.requested_capabilities {
            self.capability_grants.require(*capability)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExtensionOperation {
    AgentPackSdk,
    PluginSdk,
    RulesetPackSdk,
    ToolProviderSdk,
    Adr0008PluginBoundaries,
    ExtensionCompatibilityMatrix,
    Sdk,
    Readme,
}

impl ExtensionOperation {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AgentPackSdk => "agent_pack_sdk",
            Self::PluginSdk => "plugin_sdk",
            Self::RulesetPackSdk => "ruleset_pack_sdk",
            Self::ToolProviderSdk => "tool_provider_sdk",
            Self::Adr0008PluginBoundaries => "adr_0008_plugin_boundaries",
            Self::ExtensionCompatibilityMatrix => "extension_compatibility_matrix",
            Self::Sdk => "sdk",
            Self::Readme => "readme",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtensionCommand {
    pub operation: ExtensionOperation,
    pub reason: &'static str,
    pub evidence_path: &'static str,
    pub requested_capabilities: Vec<ExtensionCapability>,
}

impl ExtensionCommand {
    pub fn record(
        operation: ExtensionOperation,
        reason: &'static str,
        evidence_path: &'static str,
        requested_capabilities: Vec<ExtensionCapability>,
    ) -> Self {
        Self {
            operation,
            reason,
            evidence_path,
            requested_capabilities,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExtensionEventRecord {
    pub module_name: &'static str,
    pub operation: ExtensionOperation,
    pub evidence_path: &'static str,
    pub capabilities: Vec<ExtensionCapability>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExtensionEvent {
    ContractRecorded(ExtensionEventRecord),
    CompatibilityChecked(SdkCompatibilityReport),
}

pub type ExtensionEventEnvelope = EventEnvelope<ExtensionEvent>;
pub type ExtensionEventStore = EventStore<ExtensionEvent>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompatibilityResult {
    Compatible,
    Incompatible,
}

impl CompatibilityResult {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Compatible => "compatible",
            Self::Incompatible => "incompatible",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SdkCompatibilityReport {
    pub extension_id: String,
    pub ruleset_version: String,
    pub tool_schema_version: String,
    pub compatibility_result: CompatibilityResult,
    pub redacted_fields: Vec<String>,
}

impl SdkCompatibilityReport {
    pub fn compatible(
        extension_id: impl Into<String>,
        ruleset_version: impl Into<String>,
        tool_schema_version: impl Into<String>,
    ) -> Self {
        Self {
            extension_id: extension_id.into(),
            ruleset_version: ruleset_version.into(),
            tool_schema_version: tool_schema_version.into(),
            compatibility_result: CompatibilityResult::Compatible,
            redacted_fields: Vec::new(),
        }
    }

    pub fn has_required_fields(&self) -> bool {
        !self.extension_id.trim().is_empty()
            && !self.ruleset_version.trim().is_empty()
            && !self.tool_schema_version.trim().is_empty()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExtensionContract {
    pub module_name: &'static str,
    pub event_type: &'static str,
    pub operation: ExtensionOperation,
    pub read_models: &'static [&'static str],
    pub allowed_capabilities: &'static [ExtensionCapability],
    pub forbidden_capabilities: &'static [ExtensionCapability],
    pub nats_subject: &'static str,
    pub metric: &'static str,
    pub required_command_fields: &'static [&'static str],
    pub canon_boundary: &'static str,
}

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

pub fn all_extension_contracts() -> Vec<ExtensionContract> {
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
    append_extension_event(
        store,
        authority,
        command,
        contract(),
        "evidence/extensions/readme.md",
    )
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
    matches!(
        label,
        VisibilityLabel::KeeperOnly
            | VisibilityLabel::PrivateToPlayer
            | VisibilityLabel::InvestigatorPrivate
            | VisibilityLabel::AiInternal
            | VisibilityLabel::SystemOnly
            | VisibilityLabel::SystemPrivate
    )
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
macro_rules! define_extension_sdk_module {
    (
        $command:ident,
        $service:ident,
        $append_fn:ident,
        $module_name:literal,
        $event_type:literal,
        $operation:expr,
        [$($read_model:literal),* $(,)?],
        [$($capability:expr),* $(,)?],
        $evidence_path:literal
    ) => {
        pub const MODULE_NAME: &str = $module_name;
        pub const EVENT_TYPE: &str = $event_type;
        pub const READ_MODELS: &[&str] = &[$($read_model),*];
        pub const ALLOWED_CAPABILITIES: &[$crate::ExtensionCapability] = &[$($capability),*];
        pub const EVIDENCE_PATH: &str = $evidence_path;

        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct $command {
            pub inner: $crate::ExtensionCommand,
        }

        impl $command {
            pub fn record(reason: &'static str) -> Self {
                Self {
                    inner: $crate::ExtensionCommand::record(
                        $operation,
                        reason,
                        EVIDENCE_PATH,
                        ALLOWED_CAPABILITIES.to_vec(),
                    ),
                }
            }
        }

        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct $service {
            pub policy_gate: $crate::ExtensionPolicyGate,
        }

        impl $service {
            pub fn new(policy_gate: $crate::ExtensionPolicyGate) -> Self {
                Self { policy_gate }
            }

            pub fn execute(
                &self,
                store: &mut $crate::ExtensionEventStore,
                authority: &$crate::AuthorityContract,
                command: &$crate::CommandEnvelope<$command>,
            ) -> $crate::ExtensionSdkResult<$crate::ExtensionExecution> {
                self.policy_gate.authorize()?;
                let event = $append_fn(store, authority, command)?;
                Ok($crate::ExtensionExecution::from_command(
                    contract(),
                    event,
                    command,
                ))
            }
        }

        impl Default for $service {
            fn default() -> Self {
                Self::new($crate::ExtensionPolicyGate::allow(ALLOWED_CAPABILITIES))
            }
        }

        pub fn $append_fn<T>(
            store: &mut $crate::ExtensionEventStore,
            authority: &$crate::AuthorityContract,
            command: &$crate::CommandEnvelope<T>,
        ) -> $crate::KernelResult<$crate::ExtensionEventEnvelope> {
            $crate::append_extension_event(store, authority, command, contract(), EVIDENCE_PATH)
        }

        pub fn contract() -> $crate::ExtensionContract {
            $crate::ExtensionContract::new(
                MODULE_NAME,
                EVENT_TYPE,
                $operation,
                READ_MODELS,
                ALLOWED_CAPABILITIES,
            )
        }
    };
}
