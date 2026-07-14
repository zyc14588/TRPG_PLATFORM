pub mod adr_0006_openfga_opa;
pub mod audit_log_contract;
pub mod copyright_boundary;
pub mod data_retention_deletion;
pub mod permission_matrix;
pub mod policy_adapter;
pub mod policy_authorization;
pub mod policy_authz;
pub mod policy_openfga_opa;
pub mod privacy_copyright;
pub mod readme;
pub mod security_privacy;
pub mod security_privacy_copyright;
pub mod tamper_evident_audit;
pub mod visibility_enforcement_points;

use trpg_identity::{
    AuthenticationContext, CampaignMembership, CampaignRole, GlobalRole, IdentityVerifier,
    PrincipalKind,
};
use trpg_shared_kernel::shared_kernel::validate_command_envelope;
use trpg_shared_kernel::{
    ActorOrigin, ActorRole, CommandEnvelope, EventEnvelope, EventStore, KernelResult,
    PrincipalScope, TrpgError, Visibility, VisibilityLabel,
};

use crate::policy_adapter::{OpenFgaOpaPolicyAdapter, PolicyAuthorizationRequest};
use crate::tamper_evident_audit::{AuditDecision, AuditRecordDraft, AuditSink, FileAuditLog};

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
pub enum PermissionPrincipalRole {
    ServerOwner,
    CampaignOwner,
    Moderator,
    HumanKp,
    AiKp,
    Player,
    Workflow,
    RulesEngine,
    System,
    Agent,
    Provider,
    Spectator,
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
    ManageCampaignMembership,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecurityGovernanceCommand {
    pub action: SecurityGovernanceAction,
    pub target_visibility: Visibility,
    pub legal_hold: bool,
}

impl SecurityGovernanceCommand {
    pub fn new(action: SecurityGovernanceAction) -> Self {
        Self {
            action,
            target_visibility: Visibility::new(VisibilityLabel::SystemOnly),
            legal_hold: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SecurityGovernanceEvent {
    DecisionRecorded {
        module: &'static str,
        actor_id: trpg_shared_kernel::EntityId,
        principal_role: PermissionPrincipalRole,
        action: SecurityGovernanceAction,
        openfga_decision_id: String,
        openfga_policy_revision: String,
        opa_decision_id: String,
        opa_policy_revision: String,
    },
}

pub type SecurityGovernanceEventEnvelope = EventEnvelope<SecurityGovernanceEvent>;
pub type SecurityGovernanceRepository = EventStore<SecurityGovernanceEvent>;

pub fn evaluate_security_governance(
    module: &'static str,
    _repository: &mut SecurityGovernanceRepository,
    command: &CommandEnvelope<SecurityGovernanceCommand>,
) -> KernelResult<SecurityGovernanceEventEnvelope> {
    validate_security_governance_preflight(module, command)?;
    Err(TrpgError::PolicyUnavailable)
}

pub fn evaluate_security_governance_with_policy(
    module: &'static str,
    repository: &mut SecurityGovernanceRepository,
    command: &CommandEnvelope<SecurityGovernanceCommand>,
    policy: &OpenFgaOpaPolicyAdapter,
    audit: &mut FileAuditLog,
) -> KernelResult<SecurityGovernanceEventEnvelope> {
    validate_security_governance_preflight(module, command)?;
    let principal_role = principal_role_from_authenticated_actor(command)?;
    let context = command.authenticated_context();
    let request = PolicyAuthorizationRequest {
        actor_id: command.actor.id().as_str().to_owned(),
        principal_role: principal_role.as_str().to_owned(),
        campaign_id: context.resource().campaign_id().as_str().to_owned(),
        resource_type: context.resource().resource_type().as_str().to_owned(),
        resource_id: context.resource().resource_id().as_str().to_owned(),
        action: command.payload.action.as_str().to_owned(),
        authority_mode: authority_mode_name(&command.authority_mode).to_owned(),
        target_visibility: visibility_name(command.payload.target_visibility.label()).to_owned(),
        trace_id: context.trace_id().as_str().to_owned(),
    };
    if !permission_allows(
        principal_role,
        Some(&command.authority_mode),
        command.payload.action,
    ) {
        append_policy_audit(
            audit,
            command,
            &request,
            AuditDecision::Deny,
            "local-permission-deny",
            "local-permission-matrix-v1",
            "local-permission-deny",
            "local-permission-matrix-v1",
        )?;
        return Err(TrpgError::PolicyDenied);
    }

    let evidence = match policy.evaluate(&request) {
        Ok(evidence) => evidence,
        Err(error) => {
            let (openfga_revision, opa_revision) = policy.revision_snapshot();
            append_policy_audit(
                audit,
                command,
                &request,
                AuditDecision::Unavailable,
                "policy-unavailable",
                openfga_revision,
                "policy-unavailable",
                opa_revision,
            )?;
            return Err(error);
        }
    };
    evidence.validate()?;

    let allowed = evidence.openfga.allowed && evidence.opa.allowed;
    append_policy_audit(
        audit,
        command,
        &request,
        if allowed {
            AuditDecision::Permit
        } else {
            AuditDecision::Deny
        },
        &evidence.openfga.decision_id,
        &evidence.openfga.policy_revision,
        &evidence.opa.decision_id,
        &evidence.opa.policy_revision,
    )?;

    if !allowed {
        return Err(TrpgError::PolicyDenied);
    }

    repository.append(
        command,
        SECURITY_GOVERNANCE_DECISION_RECORDED_EVENT,
        SecurityGovernanceEvent::DecisionRecorded {
            module,
            actor_id: command.actor.id().clone(),
            principal_role,
            action: command.payload.action,
            openfga_decision_id: evidence.openfga.decision_id,
            openfga_policy_revision: evidence.openfga.policy_revision,
            opa_decision_id: evidence.opa.decision_id,
            opa_policy_revision: evidence.opa.policy_revision,
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub fn authorize_campaign_membership_change(
    policy: &OpenFgaOpaPolicyAdapter,
    audit: &mut FileAuditLog,
    identity_verifier: &IdentityVerifier,
    authentication: &AuthenticationContext,
    acting_membership: Option<&CampaignMembership>,
    authority_mode: &trpg_shared_kernel::AuthorityMode,
    campaign_id: &trpg_shared_kernel::EntityId,
    target_user_id: &str,
    trace_id: &str,
    now_unix_ms: u64,
) -> KernelResult<()> {
    identity_verifier
        .verify(authentication, now_unix_ms)
        .map_err(|_| TrpgError::InternalIdentityInvalid)?;
    let (principal_role, authentication_reference) = match authentication.kind() {
        PrincipalKind::UserSession {
            session_id,
            global_role: GlobalRole::ServerOwner,
        } => (PermissionPrincipalRole::ServerOwner, session_id.as_str()),
        PrincipalKind::UserSession {
            session_id,
            global_role: GlobalRole::Moderator,
        } => (PermissionPrincipalRole::Moderator, session_id.as_str()),
        PrincipalKind::UserSession {
            session_id,
            global_role: GlobalRole::User,
        } => {
            let membership = acting_membership.ok_or(TrpgError::AuthorizationDenied)?;
            if membership.user_id() != authentication.subject_id()
                || membership.campaign_id() != campaign_id
            {
                return Err(TrpgError::AuthorizationDenied);
            }
            let role = match membership.role() {
                CampaignRole::CampaignOwner => PermissionPrincipalRole::CampaignOwner,
                CampaignRole::HumanKeeper => PermissionPrincipalRole::HumanKp,
                CampaignRole::Player => PermissionPrincipalRole::Player,
                CampaignRole::Spectator => PermissionPrincipalRole::Spectator,
            };
            (role, session_id.as_str())
        }
        PrincipalKind::Workload { .. } | PrincipalKind::AgentRun { .. } => {
            return Err(TrpgError::AuthorizationDenied);
        }
    };
    if trace_id.trim().is_empty() || target_user_id.trim().is_empty() {
        return Err(TrpgError::InvalidConfiguration(
            "membership_policy_context_invalid",
        ));
    }
    let request = PolicyAuthorizationRequest {
        actor_id: authentication.subject_id().to_string(),
        principal_role: principal_role.as_str().to_owned(),
        campaign_id: campaign_id.to_string(),
        resource_type: "campaign_membership".to_owned(),
        resource_id: target_user_id.to_owned(),
        action: SecurityGovernanceAction::ManageCampaignMembership
            .as_str()
            .to_owned(),
        authority_mode: authority_mode_name(authority_mode).to_owned(),
        target_visibility: "system_only".to_owned(),
        trace_id: trace_id.to_owned(),
    };
    if !permission_allows(
        principal_role,
        Some(authority_mode),
        SecurityGovernanceAction::ManageCampaignMembership,
    ) {
        append_identity_policy_audit(
            audit,
            authentication,
            authentication_reference,
            &request,
            AuditDecision::Deny,
            "local-permission-deny",
            "local-permission-matrix-v1",
            "local-permission-deny",
            "local-permission-matrix-v1",
        )?;
        return Err(TrpgError::PolicyDenied);
    }

    let evidence = match policy.evaluate(&request) {
        Ok(evidence) => evidence,
        Err(error) => {
            let (openfga_revision, opa_revision) = policy.revision_snapshot();
            append_identity_policy_audit(
                audit,
                authentication,
                authentication_reference,
                &request,
                AuditDecision::Unavailable,
                "policy-unavailable",
                openfga_revision,
                "policy-unavailable",
                opa_revision,
            )?;
            return Err(error);
        }
    };
    evidence.validate()?;
    let allowed = evidence.openfga.allowed && evidence.opa.allowed;
    append_identity_policy_audit(
        audit,
        authentication,
        authentication_reference,
        &request,
        if allowed {
            AuditDecision::Permit
        } else {
            AuditDecision::Deny
        },
        &evidence.openfga.decision_id,
        &evidence.openfga.policy_revision,
        &evidence.opa.decision_id,
        &evidence.opa.policy_revision,
    )?;
    if allowed {
        Ok(())
    } else {
        Err(TrpgError::PolicyDenied)
    }
}

#[allow(clippy::too_many_arguments)]
fn append_identity_policy_audit(
    audit: &mut FileAuditLog,
    authentication: &AuthenticationContext,
    authentication_reference: &str,
    request: &PolicyAuthorizationRequest,
    decision: AuditDecision,
    openfga_decision_id: &str,
    openfga_policy_revision: &str,
    opa_decision_id: &str,
    opa_policy_revision: &str,
) -> KernelResult<()> {
    audit.append(AuditRecordDraft {
        actor_id: authentication.subject_id().to_string(),
        actor_origin: "user_session".to_owned(),
        authentication_reference: authentication_reference.to_owned(),
        campaign_id: request.campaign_id.clone(),
        resource_type: request.resource_type.clone(),
        resource_id: request.resource_id.clone(),
        action: request.action.clone(),
        decision,
        openfga_decision_id: openfga_decision_id.to_owned(),
        openfga_policy_revision: openfga_policy_revision.to_owned(),
        opa_decision_id: opa_decision_id.to_owned(),
        opa_policy_revision: opa_policy_revision.to_owned(),
        trace_id: request.trace_id.clone(),
    })?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn append_policy_audit(
    audit: &mut FileAuditLog,
    command: &CommandEnvelope<SecurityGovernanceCommand>,
    request: &PolicyAuthorizationRequest,
    decision: AuditDecision,
    openfga_decision_id: &str,
    openfga_policy_revision: &str,
    opa_decision_id: &str,
    opa_policy_revision: &str,
) -> KernelResult<()> {
    audit.append(AuditRecordDraft {
        actor_id: request.actor_id.clone(),
        actor_origin: actor_origin_name(command.actor.origin()).to_owned(),
        authentication_reference: authentication_reference(&command.actor),
        campaign_id: request.campaign_id.clone(),
        resource_type: request.resource_type.clone(),
        resource_id: request.resource_id.clone(),
        action: request.action.clone(),
        decision,
        openfga_decision_id: openfga_decision_id.to_owned(),
        openfga_policy_revision: openfga_policy_revision.to_owned(),
        opa_decision_id: opa_decision_id.to_owned(),
        opa_policy_revision: opa_policy_revision.to_owned(),
        trace_id: request.trace_id.clone(),
    })?;
    Ok(())
}

fn validate_security_governance_preflight(
    module: &'static str,
    command: &CommandEnvelope<SecurityGovernanceCommand>,
) -> KernelResult<()> {
    if module.trim().is_empty() {
        return Err(TrpgError::InvalidConfiguration("module_required"));
    }
    validate_command_envelope(command)?;
    if command.payload.legal_hold
        && command.payload.action == SecurityGovernanceAction::DeleteRetainedData
    {
        return Err(TrpgError::PolicyDenied);
    }
    Ok(())
}

fn principal_role_from_authenticated_actor<T>(
    command: &CommandEnvelope<T>,
) -> KernelResult<PermissionPrincipalRole> {
    if matches!(command.actor.origin(), ActorOrigin::AgentRun { .. }) {
        return Ok(PermissionPrincipalRole::Agent);
    }
    Ok(match command.actor.role() {
        ActorRole::ServerOwner => PermissionPrincipalRole::ServerOwner,
        ActorRole::CampaignOwner => PermissionPrincipalRole::CampaignOwner,
        ActorRole::HumanKeeper => PermissionPrincipalRole::HumanKp,
        ActorRole::AiKeeper => PermissionPrincipalRole::AiKp,
        ActorRole::Investigator => PermissionPrincipalRole::Player,
        ActorRole::Moderator => PermissionPrincipalRole::Moderator,
        ActorRole::Spectator => PermissionPrincipalRole::Spectator,
        ActorRole::Workflow => PermissionPrincipalRole::Workflow,
        ActorRole::RulesEngine => PermissionPrincipalRole::RulesEngine,
        ActorRole::System => PermissionPrincipalRole::System,
    })
}

impl PermissionPrincipalRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ServerOwner => "server_owner",
            Self::CampaignOwner => "campaign_owner",
            Self::Moderator => "moderator",
            Self::HumanKp => "human_kp",
            Self::AiKp => "ai_kp",
            Self::Player => "player",
            Self::Workflow => "workflow",
            Self::RulesEngine => "rules_engine",
            Self::System => "system",
            Self::Agent => "agent",
            Self::Provider => "provider",
            Self::Spectator => "spectator",
        }
    }
}

impl SecurityGovernanceAction {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PauseRoom => "pause_room",
            Self::OverrideDiceRoll => "override_dice_roll",
            Self::MutePlayer => "mute_player",
            Self::ChangeGameDecision => "change_game_decision",
            Self::ConfirmAgentDraft => "confirm_agent_draft",
            Self::RequestReconsideration => "request_reconsideration",
            Self::OverrideAiDecision => "override_ai_decision",
            Self::WriteOfficialState => "write_official_state",
            Self::ExportPlayerReport => "export_player_report",
            Self::GeneratePartySummary => "generate_party_summary",
            Self::IndexRagChunk => "index_rag_chunk",
            Self::ConnectProvider => "connect_provider",
            Self::DeleteRetainedData => "delete_retained_data",
            Self::RecordAudit => "record_audit",
            Self::ImportCopyrightedFullText => "import_copyrighted_full_text",
            Self::ManageCampaignMembership => "manage_campaign_membership",
        }
    }
}

fn actor_origin_name(origin: &ActorOrigin) -> &'static str {
    match origin {
        ActorOrigin::UserSession { .. } => "user_session",
        ActorOrigin::Workload { .. } => "workload",
        ActorOrigin::AgentRun { .. } => "agent_run",
    }
}

fn authentication_reference(actor: &trpg_shared_kernel::Actor) -> String {
    match actor.origin() {
        ActorOrigin::UserSession { session_id } => session_id.as_str().to_owned(),
        ActorOrigin::Workload { .. } => actor.id().as_str().to_owned(),
        ActorOrigin::AgentRun { run_id, .. } => run_id.as_str().to_owned(),
    }
}

fn visibility_name(label: &VisibilityLabel) -> &'static str {
    match label {
        VisibilityLabel::Public => "public",
        VisibilityLabel::PartyVisible => "party_visible",
        VisibilityLabel::KeeperOnly => "keeper_only",
        VisibilityLabel::PrivateToPlayer => "private_to_player",
        VisibilityLabel::InvestigatorPrivate => "investigator_private",
        VisibilityLabel::AiInternal => "ai_internal",
        VisibilityLabel::SystemOnly => "system_only",
        VisibilityLabel::SystemPrivate => "system_private",
    }
}

fn authority_mode_name(mode: &trpg_shared_kernel::AuthorityMode) -> &'static str {
    match mode {
        trpg_shared_kernel::AuthorityMode::HumanKp => "human_kp",
        trpg_shared_kernel::AuthorityMode::AiKp => "ai_kp",
    }
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
        (ServerOwner | CampaignOwner, ManageCampaignMembership) => true,
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
