
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
