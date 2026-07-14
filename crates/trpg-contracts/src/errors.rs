use std::fmt;

macro_rules! define_wire_error_codes {
    ($($variant:ident => $code:literal),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum WireErrorCode {
            $($variant),+
        }

        impl WireErrorCode {
            pub const ALL: &'static [Self] = &[
                $(Self::$variant),+
            ];

            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $code),+
                }
            }
        }
    };
}

define_wire_error_codes! {
    InvalidEntityId => "INVALID_ENTITY_ID",
    UnknownVisibilityLabel => "UNKNOWN_VISIBILITY_LABEL",
    MissingIdempotencyKey => "MISSING_IDEMPOTENCY_KEY",
    MissingCorrelationId => "MISSING_CORRELATION_ID",
    MissingCausationId => "MISSING_CAUSATION_ID",
    MissingFactProvenance => "MISSING_FACT_PROVENANCE",
    AuthorityViolation => "AUTHORITY_VIOLATION",
    AuthorityContractMutation => "AUTHORITY_CONTRACT_MUTATION",
    AuthorityContractImmutable => "AUTHORITY_CONTRACT_IMMUTABLE",
    AuthenticationRequired => "AUTHENTICATION_REQUIRED",
    InvalidCredentials => "INVALID_CREDENTIALS",
    SessionExpired => "SESSION_EXPIRED",
    SessionRevoked => "SESSION_REVOKED",
    AuthorizationDenied => "AUTHORIZATION_DENIED",
    CampaignScopeMismatch => "CAMPAIGN_SCOPE_MISMATCH",
    AuthorityOwnerMismatch => "AUTHORITY_OWNER_MISMATCH",
    AuthorityContractVersionConflict => "AUTHORITY_CONTRACT_VERSION_CONFLICT",
    InternalIdentityInvalid => "INTERNAL_IDENTITY_INVALID",
    PolicyUnavailable => "POLICY_UNAVAILABLE",
    PolicyEvidenceUntrusted => "POLICY_EVIDENCE_UNTRUSTED",
    DecisionConfirmationRequired => "DECISION_CONFIRMATION_REQUIRED",
    DecisionDraftChanged => "DECISION_DRAFT_CHANGED",
    DecisionExpired => "DECISION_EXPIRED",
    DecisionAlreadyCommitted => "DECISION_ALREADY_COMMITTED",
    AuditIntegrityViolation => "AUDIT_INTEGRITY_VIOLATION",
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
    InvalidConfirmedFactSource => "INVALID_CONFIRMED_FACT_SOURCE",
    MissingCommandMetadata => "MISSING_COMMAND_METADATA",
    ToolPermissionDenied => "TOOL_PERMISSION_DENIED",
    HumanKpAiDraftOnly => "HUMAN_KP_AI_DRAFT_ONLY",
    AgentToolNotAllowed => "AGENT_TOOL_NOT_ALLOWED",
    AgentDirectStateWriteForbidden => "AGENT_DIRECT_STATE_WRITE_FORBIDDEN",
    DirectLlmCallForbidden => "DIRECT_LLM_CALL_FORBIDDEN",
    PromptInjectionDetected => "PROMPT_INJECTION_DETECTED",
    LocalModelNotCertifiedForAiKp => "LOCAL_MODEL_NOT_CERTIFIED_FOR_AI_KP",
    SilentFallbackForbidden => "SILENT_FALLBACK_FORBIDDEN",
    UnauthenticatedLocalProviderExposed => "UNAUTHENTICATED_LOCAL_PROVIDER_EXPOSED",
    RagVisibilityScopeViolation => "RAG_VISIBILITY_SCOPE_VIOLATION",
    RestoreHashMismatch => "RESTORE_HASH_MISMATCH",
    RollbackRunbookRequired => "ROLLBACK_RUNBOOK_REQUIRED",
    ProjectionRebuildHashMismatch => "PROJECTION_REBUILD_HASH_MISMATCH",
    IdempotencyKeyRequired => "IDEMPOTENCY_KEY_REQUIRED",
    RealtimeVisibilityViolation => "REALTIME_VISIBILITY_VIOLATION",
    NatsSubjectContractViolation => "NATS_SUBJECT_CONTRACT_VIOLATION",
    IdempotencyContractBroken => "IDEMPOTENCY_CONTRACT_BROKEN",
    ClientFormalDiceForbidden => "CLIENT_FORMAL_DICE_FORBIDDEN",
    AiDiceFabricationForbidden => "AI_DICE_FABRICATION_FORBIDDEN",
    StateChangeWithoutEvent => "STATE_CHANGE_WITHOUT_EVENT",
    EventContractUnknown => "EVENT_CONTRACT_UNKNOWN",
    EventContractVersionMismatch => "EVENT_CONTRACT_VERSION_MISMATCH",
    ServiceConfigurationInvalid => "SERVICE_CONFIGURATION_INVALID",
    ServiceInitializationFailed => "SERVICE_INITIALIZATION_FAILED",
    ExtensionStateWriteForbidden => "EXTENSION_STATE_WRITE_FORBIDDEN",
    ExtensionDirectLlmForbidden => "EXTENSION_DIRECT_LLM_FORBIDDEN",
    ExtensionDatabaseWriteForbidden => "EXTENSION_DATABASE_WRITE_FORBIDDEN",
    ExtensionToolGateBypassForbidden => "EXTENSION_TOOL_GATE_BYPASS_FORBIDDEN",
    ExtensionAuthorityContractForbidden => "EXTENSION_AUTHORITY_CONTRACT_FORBIDDEN",
    ExtensionDiceForgeForbidden => "EXTENSION_DICE_FORGE_FORBIDDEN",
    ExtensionVisibilityLeakForbidden => "EXTENSION_VISIBILITY_LEAK_FORBIDDEN",
    ExtensionCapabilityDenied => "EXTENSION_CAPABILITY_DENIED",
    ExtensionToolGrantDenied => "EXTENSION_TOOL_GRANT_DENIED",
    ExtensionOpenfgaDenied => "EXTENSION_OPENFGA_DENIED",
    ExtensionOpaDenied => "EXTENSION_OPA_DENIED",
    ExtensionAuditRequired => "EXTENSION_AUDIT_REQUIRED",
    ExtensionCompatibilityFieldsMissing => "EXTENSION_COMPATIBILITY_FIELDS_MISSING",
    ExtensionCompatibilityRejected => "EXTENSION_COMPATIBILITY_REJECTED",
}

impl WireErrorCode {
    pub fn is_screaming_snake_case(self) -> bool {
        let code = self.as_str().as_bytes();
        !code.is_empty()
            && code[0].is_ascii_uppercase()
            && code[code.len() - 1] != b'_'
            && code
                .iter()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || *byte == b'_')
            && !code.windows(2).any(|pair| pair == b"__")
    }
}

impl fmt::Display for WireErrorCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}
