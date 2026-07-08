pub mod adr_0006_openfga_opa;
pub mod audit_log_contract;
pub mod copyright_boundary;
pub mod data_retention_deletion;
pub mod permission_matrix;
pub mod policy_authorization;
pub mod policy_authz;
pub mod policy_openfga_opa;
pub mod privacy_copyright;
pub mod readme;
pub mod security_privacy;
pub mod security_privacy_copyright;
pub mod visibility_enforcement_points;

use trpg_shared_kernel::{
    CommandEnvelope, EventEnvelope, EventStore, KernelResult, PrincipalScope, TrpgError,
    Visibility, VisibilityLabel,
};

pub const SECURITY_GOVERNANCE_DECISION_RECORDED_EVENT: &str =
    "security_governance.decision_recorded";
pub const SECURITY_GOVERNANCE_METRIC_MODULE: &str = "security_governance";
pub const SECURITY_GOVERNANCE_REQUIRED_METRICS: &[&str] = &[
    "trpg_command_total",
    "trpg_event_append_latency_ms",
    "trpg_policy_deny_total",
    "trpg_visibility_redaction_total",
    "trpg_audit_event_total",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyGateDecision {
    Permit,
    Deny,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PermissionPrincipalRole {
    ServerOwner,
    Moderator,
    HumanKp,
    AiKp,
    Player,
    Workflow,
    RulesEngine,
    System,
    Agent,
    Provider,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecurityGovernanceAction {
    PauseRoom,
    OverrideDiceRoll,
    MutePlayer,
    ChangeGameDecision,
    ConfirmAgentDraft,
    RequestReconsideration,
    OverrideAiDecision,
    WriteOfficialState,
    ExportPlayerReport,
    GeneratePartySummary,
    IndexRagChunk,
    ConnectProvider,
    DeleteRetainedData,
    RecordAudit,
    ImportCopyrightedFullText,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecurityGovernanceCommand {
    pub principal_role: PermissionPrincipalRole,
    pub action: SecurityGovernanceAction,
    pub openfga_decision: PolicyGateDecision,
    pub opa_decision: PolicyGateDecision,
    pub target_visibility: Visibility,
    pub legal_hold: bool,
}

impl SecurityGovernanceCommand {
    pub fn new(principal_role: PermissionPrincipalRole, action: SecurityGovernanceAction) -> Self {
        Self {
            principal_role,
            action,
            openfga_decision: PolicyGateDecision::Permit,
            opa_decision: PolicyGateDecision::Permit,
            target_visibility: Visibility::new(VisibilityLabel::SystemOnly),
            legal_hold: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SecurityGovernanceEvent {
    DecisionRecorded {
        module: &'static str,
        principal_role: PermissionPrincipalRole,
        action: SecurityGovernanceAction,
    },
}

pub type SecurityGovernanceEventEnvelope = EventEnvelope<SecurityGovernanceEvent>;
pub type SecurityGovernanceRepository = EventStore<SecurityGovernanceEvent>;

pub fn evaluate_security_governance(
    module: &'static str,
    repository: &mut SecurityGovernanceRepository,
    command: &CommandEnvelope<SecurityGovernanceCommand>,
) -> KernelResult<SecurityGovernanceEventEnvelope> {
    if module.trim().is_empty() {
        return Err(TrpgError::InvalidConfiguration("module_required"));
    }
    if command.payload.openfga_decision == PolicyGateDecision::Deny
        || command.payload.opa_decision == PolicyGateDecision::Deny
    {
        return Err(TrpgError::PolicyDenied);
    }
    if command.payload.legal_hold
        && command.payload.action == SecurityGovernanceAction::DeleteRetainedData
    {
        return Err(TrpgError::PolicyDenied);
    }
    if !permission_allows(
        command.payload.principal_role,
        Some(&command.authority_mode),
        command.payload.action,
    ) {
        return Err(TrpgError::PolicyDenied);
    }

    repository.append(
        command,
        SECURITY_GOVERNANCE_DECISION_RECORDED_EVENT,
        SecurityGovernanceEvent::DecisionRecorded {
            module,
            principal_role: command.payload.principal_role,
            action: command.payload.action,
        },
    )
}

pub fn permission_allows(
    role: PermissionPrincipalRole,
    authority_mode: Option<&trpg_shared_kernel::AuthorityMode>,
    action: SecurityGovernanceAction,
) -> bool {
    use trpg_shared_kernel::AuthorityMode;
    use PermissionPrincipalRole::*;
    use SecurityGovernanceAction::*;

    match (role, action) {
        (ServerOwner, PauseRoom) => true,
        (ServerOwner, OverrideDiceRoll) => false,
        (Moderator, MutePlayer) => true,
        (Moderator, ChangeGameDecision) => false,
        (HumanKp, ConfirmAgentDraft) => {
            matches!(authority_mode, Some(mode) if *mode == AuthorityMode::HumanKp)
        }
        (Player, RequestReconsideration) => {
            matches!(authority_mode, Some(mode) if *mode == AuthorityMode::AiKp)
        }
        (Player, OverrideAiDecision) => false,
        (Workflow | RulesEngine | System, WriteOfficialState | RecordAudit) => true,
        (Workflow | System, DeleteRetainedData) => true,
        (Workflow | System, ExportPlayerReport | GeneratePartySummary | IndexRagChunk) => true,
        (System, ConnectProvider) => true,
        (Agent | Provider, WriteOfficialState) => false,
        (_, ImportCopyrightedFullText) => false,
        _ => false,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DerivedObject {
    PlayerExport,
    PartySummary,
    RagChunk,
    DebugLog,
    AgentContext,
    AuditLog,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RedactionOutcome {
    Visible,
    Redacted,
    Omitted,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RedactionDecision {
    pub outcome: RedactionOutcome,
    pub result_visibility: VisibilityLabel,
    pub error_code: Option<&'static str>,
}

pub fn evaluate_visibility_derivation(
    source: &Visibility,
    principal: &PrincipalScope,
    target: DerivedObject,
) -> RedactionDecision {
    if *source.label() == VisibilityLabel::AiInternal
        && matches!(
            target,
            DerivedObject::PlayerExport | DerivedObject::PartySummary | DerivedObject::RagChunk
        )
    {
        return RedactionDecision {
            outcome: RedactionOutcome::Redacted,
            result_visibility: VisibilityLabel::AiInternal,
            error_code: Some("AI_INTERNAL_EXPORT_FORBIDDEN"),
        };
    }

    if source.can_view(principal) {
        return RedactionDecision {
            outcome: RedactionOutcome::Visible,
            result_visibility: source.label().clone(),
            error_code: None,
        };
    }

    let error_code = match (source.label(), target) {
        (VisibilityLabel::KeeperOnly, DerivedObject::PlayerExport) => {
            "VISIBILITY_DOWNGRADE_FORBIDDEN"
        }
        (VisibilityLabel::PrivateToPlayer, DerivedObject::PartySummary) => {
            "VISIBILITY_SCOPE_VIOLATION"
        }
        _ => "VISIBILITY_LEAKAGE_DETECTED",
    };
    let outcome = if target == DerivedObject::RagChunk {
        RedactionOutcome::Omitted
    } else {
        RedactionOutcome::Redacted
    };

    RedactionDecision {
        outcome,
        result_visibility: source.label().clone(),
        error_code: Some(error_code),
    }
}

pub fn most_restrictive_visibility(labels: &[VisibilityLabel]) -> VisibilityLabel {
    labels
        .iter()
        .max_by_key(|label| visibility_rank(label))
        .cloned()
        .unwrap_or(VisibilityLabel::Public)
}

fn visibility_rank(label: &VisibilityLabel) -> u8 {
    match label {
        VisibilityLabel::Public => 0,
        VisibilityLabel::PartyVisible => 1,
        VisibilityLabel::PrivateToPlayer | VisibilityLabel::InvestigatorPrivate => 2,
        VisibilityLabel::KeeperOnly => 3,
        VisibilityLabel::AiInternal => 4,
        VisibilityLabel::SystemOnly | VisibilityLabel::SystemPrivate => 5,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeploymentEnvironment {
    Development,
    Production,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderEndpoint {
    pub provider_type: String,
    pub base_url: String,
    pub api_key: String,
    pub environment: DeploymentEnvironment,
    pub authenticated: bool,
}

pub fn validate_provider_boundary(endpoint: &ProviderEndpoint) -> KernelResult<()> {
    if endpoint.environment == DeploymentEnvironment::Production
        && endpoint.base_url.contains("0.0.0.0")
        && !endpoint.authenticated
    {
        return Err(TrpgError::InvalidConfiguration(
            "unauthenticated_local_provider_exposed",
        ));
    }
    if endpoint.environment == DeploymentEnvironment::Production
        && is_placeholder_api_key(&endpoint.api_key)
    {
        return Err(TrpgError::InvalidConfiguration("placeholder_api_key"));
    }

    Ok(())
}

pub fn is_placeholder_api_key(api_key: &str) -> bool {
    matches!(
        api_key.trim(),
        "" | "ollama" | "sk-no-key-required" | "changeme" | "placeholder"
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LocalModelCertificationLevel {
    LocalModelLevel1,
    LocalModelLevel4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LocalModelCertificationInput {
    pub json_schema_support: bool,
    pub tool_call_support: bool,
    pub visibility_tests_pass: bool,
    pub rules_eval_pass: bool,
    pub latency_ms: u64,
}

pub fn certify_local_model(input: LocalModelCertificationInput) -> LocalModelCertificationLevel {
    if input.json_schema_support
        && input.tool_call_support
        && input.visibility_tests_pass
        && input.rules_eval_pass
        && input.latency_ms <= 2_000
    {
        LocalModelCertificationLevel::LocalModelLevel4
    } else {
        LocalModelCertificationLevel::LocalModelLevel1
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CloudFallbackDecision {
    Allow,
    DenyAndAudit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CloudFallbackRequest {
    pub cloud_fallback_enabled: bool,
    pub cloud_call_attempted: bool,
    pub user_notice: bool,
    pub snapshot_recorded: bool,
}

pub fn evaluate_cloud_fallback(request: CloudFallbackRequest) -> CloudFallbackDecision {
    if request.cloud_call_attempted && !request.cloud_fallback_enabled {
        return CloudFallbackDecision::DenyAndAudit;
    }
    if request.cloud_fallback_enabled && request.user_notice && request.snapshot_recorded {
        CloudFallbackDecision::Allow
    } else {
        CloudFallbackDecision::DenyAndAudit
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentLicense {
    Original,
    Permissive,
    CopyrightedCommercial,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContentUse {
    ShortQuote,
    FullTextImport,
    PlayerExport,
    PrivateReference,
}

pub fn copyright_allows(license: ContentLicense, use_case: ContentUse) -> bool {
    !matches!(
        (license, use_case),
        (
            ContentLicense::CopyrightedCommercial,
            ContentUse::FullTextImport | ContentUse::PlayerExport
        )
    )
}
