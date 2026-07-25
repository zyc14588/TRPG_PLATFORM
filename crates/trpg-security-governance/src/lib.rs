pub mod adr_0006_openfga_opa;
pub mod audit_log_contract;
pub mod cloud_egress;
pub mod copyright_boundary;
pub mod data_retention_deletion;
pub mod derived_visibility;
pub mod formal_commit_audit;
pub mod permission_matrix;
pub mod policy_adapter;
pub mod policy_authorization;
pub mod policy_authz;
pub mod policy_openfga_opa;
pub mod privacy_copyright;
pub mod readme;
pub mod secret;
pub mod security_privacy;
pub mod security_privacy_copyright;
pub mod tamper_evident_audit;
pub mod visibility_enforcement_points;

use sha2::{Digest, Sha256};
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

pub use cloud_egress::{
    authorize_cloud_egress, CloudEgressAttempt, CloudEgressAuthorization, CloudEgressDecision,
    CloudEgressDenial, CloudEgressLedger, CloudEgressOutcome, CloudEgressRequest,
};
pub use derived_visibility::{
    evaluate_derived_visibility, DerivationRequest, DerivedVisibilityDecision,
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
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

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
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
    DeletePersonalData,
    RecordAudit,
    ImportCopyrightedFullText,
    ManageCampaignMembership,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SecurityGovernanceCommand {
    pub action: SecurityGovernanceAction,
    pub target_visibility: Visibility,
}

impl SecurityGovernanceCommand {
    pub fn new(action: SecurityGovernanceAction) -> Self {
        Self {
            action,
            target_visibility: Visibility::new(VisibilityLabel::SystemOnly),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
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

#[derive(Clone, Copy)]
pub struct PolicyIdentityContext<'a> {
    verifier: &'a IdentityVerifier,
    authentication: &'a AuthenticationContext,
    now_unix_ms: u64,
}

impl<'a> PolicyIdentityContext<'a> {
    pub const fn new(
        verifier: &'a IdentityVerifier,
        authentication: &'a AuthenticationContext,
        now_unix_ms: u64,
    ) -> Self {
        Self {
            verifier,
            authentication,
            now_unix_ms,
        }
    }
}

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
    identity: PolicyIdentityContext<'_>,
) -> KernelResult<SecurityGovernanceEventEnvelope> {
    validate_security_governance_preflight(module, command)?;
    identity
        .verifier
        .verify_actor(
            identity.authentication,
            &command.actor,
            command.authenticated_context().resource().campaign_id(),
            identity.now_unix_ms,
        )
        .map_err(|_| TrpgError::InternalIdentityInvalid)?;
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
        requested_role: None,
        target_visibility: visibility_name(command.payload.target_visibility.label()).to_owned(),
        target_visibility_subject: command
            .payload
            .target_visibility
            .subject_id()
            .map(ToString::to_string),
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
    audit: &mut impl AuditSink,
    identity_verifier: &IdentityVerifier,
    authentication: &AuthenticationContext,
    acting_membership: Option<&CampaignMembership>,
    authority_mode: &trpg_shared_kernel::AuthorityMode,
    campaign_id: &trpg_shared_kernel::EntityId,
    target_user_id: &str,
    requested_role: CampaignRole,
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
        requested_role: Some(campaign_role_name(requested_role).to_owned()),
        target_visibility: "system_only".to_owned(),
        target_visibility_subject: None,
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
    audit: &mut impl AuditSink,
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
        requested_role: request
            .requested_role
            .clone()
            .unwrap_or_else(|| "not_applicable".to_owned()),
        visibility_label: request.target_visibility.clone(),
        visibility_subject: request
            .target_visibility_subject
            .clone()
            .unwrap_or_else(|| "not_applicable".to_owned()),
        provenance_kind: "tool_result".to_owned(),
        provenance_reference: request.trace_id.clone(),
        provenance_recorded_by: "policy_adapter".to_owned(),
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
        requested_role: request
            .requested_role
            .clone()
            .unwrap_or_else(|| "not_applicable".to_owned()),
        visibility_label: visibility_name(command.visibility.label()).to_owned(),
        visibility_subject: command
            .visibility
            .subject_id()
            .map(ToString::to_string)
            .unwrap_or_else(|| "not_applicable".to_owned()),
        provenance_kind: provenance_kind_name(&command.fact_provenance.kind).to_owned(),
        provenance_reference: command.fact_provenance.reference.to_string(),
        provenance_recorded_by: command.fact_provenance.recorded_by.to_string(),
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
    if !command.payload.target_visibility.is_well_formed() {
        return Err(TrpgError::VisibilityDenied);
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
            Self::DeletePersonalData => "delete_personal_data",
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
    label.as_str()
}

fn provenance_kind_name(kind: &trpg_shared_kernel::ProvenanceKind) -> &'static str {
    match kind {
        trpg_shared_kernel::ProvenanceKind::UserStatement => "user_statement",
        trpg_shared_kernel::ProvenanceKind::HumanKeeperStatement => "human_keeper_statement",
        trpg_shared_kernel::ProvenanceKind::RulesEngineDecision => "rules_engine_decision",
        trpg_shared_kernel::ProvenanceKind::ToolResult => "tool_result",
        trpg_shared_kernel::ProvenanceKind::AgentProposal => "agent_proposal",
        trpg_shared_kernel::ProvenanceKind::ImportedSource => "imported_source",
        trpg_shared_kernel::ProvenanceKind::SystemFixture => "system_fixture",
    }
}

fn campaign_role_name(role: CampaignRole) -> &'static str {
    match role {
        CampaignRole::CampaignOwner => "campaign_owner",
        CampaignRole::HumanKeeper => "human_keeper",
        CampaignRole::Player => "player",
        CampaignRole::Spectator => "spectator",
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
        (Workflow | System, DeletePersonalData) => true,
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
    let sources = [source.clone()];
    let decision = evaluate_derived_visibility(DerivationRequest {
        sources: &sources,
        // This compatibility facade represents the trusted governance worker.
        // The supplied principal remains exclusively the output audience;
        // processor authority cannot make the result player-visible.
        processor: &PrincipalScope::System,
        target_audience: principal,
        target,
    });
    RedactionDecision {
        outcome: decision.outcome,
        result_visibility: decision.result_visibility.label().clone(),
        error_code: decision.error_code,
    }
}

pub fn most_restrictive_visibility(labels: &[VisibilityLabel]) -> VisibilityLabel {
    labels
        .iter()
        .cloned()
        .reduce(|current, candidate| current.conservative_merge(&candidate))
        .unwrap_or(VisibilityLabel::Public)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeploymentEnvironment {
    Development,
    Production,
}

impl DeploymentEnvironment {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Production => "production",
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ProviderEndpoint {
    provider_type: String,
    base_url: String,
    credential: secret::SecretReference,
    environment: DeploymentEnvironment,
    model_id: String,
    model_artifact_sha256: String,
}

impl ProviderEndpoint {
    pub fn new(
        provider_type: impl Into<String>,
        base_url: impl Into<String>,
        credential: secret::SecretReference,
        environment: DeploymentEnvironment,
        model_id: impl Into<String>,
        model_artifact_sha256: impl Into<String>,
    ) -> KernelResult<Self> {
        let endpoint = Self {
            provider_type: provider_type.into(),
            base_url: base_url.into(),
            credential,
            environment,
            model_id: model_id.into(),
            model_artifact_sha256: model_artifact_sha256.into(),
        };
        endpoint.validate_model_identity()?;
        Ok(endpoint)
    }

    pub fn provider_type(&self) -> &str {
        &self.provider_type
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn credential(&self) -> &secret::SecretReference {
        &self.credential
    }

    pub const fn environment(&self) -> DeploymentEnvironment {
        self.environment
    }

    pub fn model_id(&self) -> &str {
        &self.model_id
    }

    pub fn model_artifact_sha256(&self) -> &str {
        &self.model_artifact_sha256
    }

    fn validate_model_identity(&self) -> KernelResult<()> {
        if self.model_id.trim().is_empty()
            || self.model_id.len() > 256
            || self.model_artifact_sha256.len() != 71
            || !self.model_artifact_sha256.starts_with("sha256:")
            || !self.model_artifact_sha256[7..]
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(TrpgError::InvalidConfiguration(
                "provider_model_identity_invalid",
            ));
        }
        Ok(())
    }
}

impl std::fmt::Debug for ProviderEndpoint {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProviderEndpoint")
            .field("provider_type", &self.provider_type)
            .field("base_url", &"[redacted endpoint]")
            .field("credential", &self.credential)
            .field("environment", &self.environment)
            .field("model_id", &self.model_id)
            .field("model_artifact_sha256", &self.model_artifact_sha256)
            .finish()
    }
}

/// Opaque proof that the exact provider endpoint, model artifact and active
/// secret version were validated together. Callers can persist only the
/// digest; they cannot manufacture an "authenticated" boolean.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderBoundaryAttestation {
    security_snapshot_digest: String,
}

impl ProviderBoundaryAttestation {
    pub fn security_snapshot_digest(&self) -> &str {
        &self.security_snapshot_digest
    }
}

pub fn validate_provider_boundary<R: secret::SecretResolver>(
    endpoint: &ProviderEndpoint,
    secret_manager: &secret::SecretManager<R>,
) -> KernelResult<ProviderBoundaryAttestation> {
    endpoint.validate_model_identity()?;
    if endpoint.environment == DeploymentEnvironment::Production
        && !endpoint.credential.production_eligible()
    {
        return Err(TrpgError::InvalidConfiguration(
            "production_secret_backend_required",
        ));
    }
    let local_provider = match endpoint.provider_type.trim().to_ascii_lowercase().as_str() {
        "cloud" | "cloud-provider" | "openai" | "anthropic" => false,
        "ollama"
        | "llama_cpp"
        | "llama.cpp"
        | "local-model-provider"
        | "local-openai-compatible" => true,
        _ => {
            return Err(TrpgError::InvalidConfiguration(
                "unknown_provider_classification",
            ))
        }
    };
    let base_url = url::Url::parse(&endpoint.base_url)
        .map_err(|_| TrpgError::InvalidConfiguration("provider_endpoint_invalid"))?;
    if base_url.host_str().is_none()
        || !base_url.username().is_empty()
        || base_url.password().is_some()
        || base_url.query().is_some()
        || base_url.fragment().is_some()
    {
        return Err(TrpgError::InvalidConfiguration(
            "provider_endpoint_must_not_contain_credentials",
        ));
    }
    let host_is_loopback = matches!(base_url.host_str(), Some("localhost" | "127.0.0.1" | "::1"));
    if local_provider && !host_is_loopback {
        return Err(TrpgError::InvalidConfiguration(
            "unauthenticated_local_provider_exposed",
        ));
    }
    if !local_provider && host_is_loopback {
        return Err(TrpgError::InvalidConfiguration(
            "cloud_provider_endpoint_must_be_remote",
        ));
    }
    if endpoint.environment == DeploymentEnvironment::Production && base_url.scheme() != "https" {
        return Err(TrpgError::InvalidConfiguration(
            "production_provider_https_required",
        ));
    }

    // Resolution is deliberately performed at the configuration boundary.
    // Merely naming a production-capable backend does not prove that the
    // version exists, remains active, or can be decrypted/read.
    let resolved_secret = secret_manager.resolve(&endpoint.credential)?;
    drop(resolved_secret);

    let mut digest = Sha256::new();
    append_provider_snapshot_field(&mut digest, b"trpg-provider-security-snapshot-v1");
    append_provider_snapshot_field(&mut digest, endpoint.environment.as_str().as_bytes());
    append_provider_snapshot_field(
        &mut digest,
        endpoint
            .provider_type
            .trim()
            .to_ascii_lowercase()
            .as_bytes(),
    );
    append_provider_snapshot_field(&mut digest, base_url.as_str().as_bytes());
    append_provider_snapshot_field(&mut digest, endpoint.model_id.as_bytes());
    append_provider_snapshot_field(
        &mut digest,
        endpoint
            .model_artifact_sha256
            .to_ascii_lowercase()
            .as_bytes(),
    );
    append_provider_snapshot_field(
        &mut digest,
        match endpoint.credential.backend() {
            secret::SecretBackend::Kms => b"kms",
            secret::SecretBackend::MountedFile => b"mounted_file",
            secret::SecretBackend::DevelopmentMemory => b"development_memory",
        },
    );
    append_provider_snapshot_field(&mut digest, endpoint.credential.secret_id().as_bytes());
    append_provider_snapshot_field(
        &mut digest,
        endpoint.credential.version().to_string().as_bytes(),
    );

    Ok(ProviderBoundaryAttestation {
        security_snapshot_digest: format!("sha256:{:x}", digest.finalize()),
    })
}

fn append_provider_snapshot_field(digest: &mut Sha256, value: &[u8]) {
    digest.update((value.len() as u64).to_be_bytes());
    digest.update(value);
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
