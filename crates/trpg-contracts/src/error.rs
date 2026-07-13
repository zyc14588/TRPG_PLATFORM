use std::error::Error;
use std::fmt;
use std::str::FromStr;

macro_rules! wire_error_codes {
    ($($variant:ident => $code:literal),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum WireErrorCode {
            $($variant),+
        }

        impl WireErrorCode {
            pub const ALL: &'static [Self] = &[$(Self::$variant),+];

            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $code),+
                }
            }
        }
    };
}

wire_error_codes! {
    InvalidEntityId => "INVALID_ENTITY_ID",
    UnknownVisibilityLabel => "UNKNOWN_VISIBILITY_LABEL",
    MissingIdempotencyKey => "MISSING_IDEMPOTENCY_KEY",
    MissingCorrelationId => "MISSING_CORRELATION_ID",
    MissingCausationId => "MISSING_CAUSATION_ID",
    MissingFactProvenance => "MISSING_FACT_PROVENANCE",
    AuthorityViolation => "AUTHORITY_VIOLATION",
    AuthorityContractMutation => "AUTHORITY_CONTRACT_MUTATION",
    DirectAgentStateWrite => "DIRECT_AGENT_STATE_WRITE",
    PolicyDenied => "POLICY_DENIED",
    ExpectedVersionConflict => "EXPECTED_VERSION_CONFLICT",
    DuplicateCommand => "DUPLICATE_COMMAND",
    VisibilityDenied => "VISIBILITY_DENIED",
    InvalidConfiguration => "INVALID_CONFIGURATION",
    DependencyDirectionViolation => "DEPENDENCY_DIRECTION_VIOLATION",
    CrateOwnershipViolation => "CRATE_OWNERSHIP_VIOLATION",
    WorkspaceContractViolation => "WORKSPACE_CONTRACT_VIOLATION",
    RustCodingPolicyViolation => "RUST_CODING_POLICY_VIOLATION",
    OpenSourceReferenceViolation => "OPEN_SOURCE_REFERENCE_VIOLATION",
    ToolPermissionDenied => "TOOL_PERMISSION_DENIED",
    HumanKpAiDraftOnly => "HUMAN_KP_AI_DRAFT_ONLY",
    AgentDirectStateWriteForbidden => "AGENT_DIRECT_STATE_WRITE_FORBIDDEN",
    DirectLlmCallForbidden => "DIRECT_LLM_CALL_FORBIDDEN",
    PromptInjectionDetected => "PROMPT_INJECTION_DETECTED",
    LocalModelNotCertifiedForAiKp => "LOCAL_MODEL_NOT_CERTIFIED_FOR_AI_KP",
    SilentFallbackForbidden => "SILENT_FALLBACK_FORBIDDEN",
    UnauthenticatedLocalProviderExposed => "UNAUTHENTICATED_LOCAL_PROVIDER_EXPOSED",
    RagVisibilityScopeViolation => "RAG_VISIBILITY_SCOPE_VIOLATION",
    AuthorityContractImmutable => "AUTHORITY_CONTRACT_IMMUTABLE",
    InvalidConfirmedFactSource => "INVALID_CONFIRMED_FACT_SOURCE",
    MissingCommandMetadata => "MISSING_COMMAND_METADATA",
    AgentToolNotAllowed => "AGENT_TOOL_NOT_ALLOWED",
    RestoreHashMismatch => "RESTORE_HASH_MISMATCH",
    RollbackRunbookRequired => "ROLLBACK_RUNBOOK_REQUIRED",
    ProjectionRebuildHashMismatch => "PROJECTION_REBUILD_HASH_MISMATCH",
    ExtensionStateWriteForbidden => "EXTENSION_STATE_WRITE_FORBIDDEN",
    ExtensionDirectLlmForbidden => "EXTENSION_DIRECT_LLM_FORBIDDEN",
    ExtensionDatabaseWriteForbidden => "EXTENSION_DATABASE_WRITE_FORBIDDEN",
    ExtensionToolGateBypassForbidden => "EXTENSION_TOOL_GATE_BYPASS_FORBIDDEN",
    ExtensionAuthorityContractForbidden => "EXTENSION_AUTHORITY_CONTRACT_FORBIDDEN",
    ExtensionDiceForgeForbidden => "EXTENSION_DICE_FORGE_FORBIDDEN",
    ExtensionVisibilityLeakForbidden => "EXTENSION_VISIBILITY_LEAK_FORBIDDEN",
    ExtensionCapabilityDenied => "EXTENSION_CAPABILITY_DENIED",
    ExtensionToolGrantDenied => "EXTENSION_TOOL_GRANT_DENIED",
    ExtensionOpenFgaDenied => "EXTENSION_OPENFGA_DENIED",
    ExtensionOpaDenied => "EXTENSION_OPA_DENIED",
    ExtensionAuditRequired => "EXTENSION_AUDIT_REQUIRED",
    ExtensionCompatibilityFieldsMissing => "EXTENSION_COMPATIBILITY_FIELDS_MISSING",
    ExtensionCompatibilityRejected => "EXTENSION_COMPATIBILITY_REJECTED",
}

impl WireErrorCode {
    pub fn lookup(value: &str) -> Result<Self, UnknownWireErrorCode> {
        Self::ALL
            .iter()
            .copied()
            .find(|code| code.as_str() == value)
            .ok_or(UnknownWireErrorCode)
    }
}

impl fmt::Display for WireErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for WireErrorCode {
    type Err = UnknownWireErrorCode;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::lookup(value)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UnknownWireErrorCode;

impl fmt::Display for UnknownWireErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("unknown wire error code")
    }
}

impl Error for UnknownWireErrorCode {}
