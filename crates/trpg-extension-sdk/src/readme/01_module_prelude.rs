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
    pub fn code(&self) -> &'static str {
        match self {
            Self::ForbiddenCapability(code) | Self::CompatibilityRejected(code) => code.as_str(),
            Self::Kernel(error) => error.code(),
        }
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
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
    capability_grants: ExtensionCapabilityGrantSet,
    requested_capabilities: Vec<ExtensionCapability>,
}

impl ExtensionPolicyGate {
    pub fn with_capability_grants(
        grants: ExtensionCapabilityGrantSet,
        requested_capabilities: &[ExtensionCapability],
    ) -> ExtensionSdkResult<Self> {
        for capability in requested_capabilities {
            grants.require(*capability)?;
        }
        Ok(Self {
            capability_grants: grants,
            requested_capabilities: requested_capabilities.to_vec(),
        })
    }

    pub fn default_deny(requested_capabilities: &[ExtensionCapability]) -> Self {
        Self {
            capability_grants: ExtensionCapabilityGrantSet::default(),
            requested_capabilities: requested_capabilities.to_vec(),
        }
    }

    pub fn authorize(&self) -> ExtensionSdkResult<()> {
        for capability in &self.requested_capabilities {
            self.capability_grants.require(*capability)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
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

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct ExtensionEventRecord {
    pub module_name: &'static str,
    pub operation: ExtensionOperation,
    pub evidence_path: &'static str,
    pub capabilities: Vec<ExtensionCapability>,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub enum ExtensionEvent {
    ContractRecorded(ExtensionEventRecord),
    CompatibilityChecked(SdkCompatibilityReport),
}

pub type ExtensionEventEnvelope = EventEnvelope<ExtensionEvent>;
pub type ExtensionEventStore = EventStore<ExtensionEvent>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
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

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
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
