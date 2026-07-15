use std::path::Path;
use std::sync::{Arc, Mutex};

use trpg_identity::{AuthenticationContext, IdentityVerifier, PrincipalKind};
use trpg_shared_kernel::{
    ActorRole, AuthorityContract, AuthorityMode, CanonicalPolicyAudit, CommandEnvelope,
    KernelResult, TrpgError, VisibilityLabel,
};

use crate::policy_adapter::{OpenFgaOpaPolicyAdapter, PolicyAuthorizationRequest, PolicyEvidence};
use crate::tamper_evident_audit::{
    AuditDecision, AuditRecord, AuditRecordDraft, AuditSink, FileAuditLog,
};

/// Cloneable handle to the external tamper-evident witness used by formal commits.
#[derive(Clone)]
pub struct FormalCommitAudit {
    inner: Arc<Mutex<FileAuditLog>>,
}

impl std::fmt::Debug for FormalCommitAudit {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FormalCommitAudit")
            .field("inner", &"[REDACTED]")
            .finish()
    }
}

impl FormalCommitAudit {
    pub fn from_file_log(log: FileAuditLog) -> Self {
        Self {
            inner: Arc::new(Mutex::new(log)),
        }
    }

    pub fn open(
        path: impl AsRef<Path>,
        integrity_key_id: impl Into<String>,
        integrity_key: &[u8],
    ) -> KernelResult<Self> {
        Ok(Self {
            inner: Arc::new(Mutex::new(FileAuditLog::open(
                path,
                integrity_key_id,
                integrity_key,
            )?)),
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn record_policy_decision<T>(
        &self,
        authentication: &AuthenticationContext,
        command: &CommandEnvelope<T>,
        request: &PolicyAuthorizationRequest,
        requested_role: &str,
        decision: AuditDecision,
        openfga_decision_id: &str,
        openfga_policy_revision: &str,
        opa_decision_id: &str,
        opa_policy_revision: &str,
    ) -> KernelResult<AuditRecord> {
        let (actor_origin, authentication_reference) = match authentication.kind() {
            PrincipalKind::UserSession { session_id, .. } => ("user_session", session_id.as_str()),
            PrincipalKind::Workload { .. } => ("workload", authentication.subject_id().as_str()),
            PrincipalKind::AgentRun { run_id, .. } => ("agent_run", run_id.as_str()),
        };
        let context = command.authenticated_context();
        let draft = AuditRecordDraft {
            actor_id: authentication.subject_id().to_string(),
            actor_origin: actor_origin.to_owned(),
            authentication_reference: authentication_reference.to_owned(),
            campaign_id: context.resource().campaign_id().to_string(),
            resource_type: context.resource().resource_type().to_string(),
            resource_id: context.resource().resource_id().to_string(),
            action: request.action.clone(),
            requested_role: requested_role.to_owned(),
            visibility_label: visibility_name(command.visibility.label()).to_owned(),
            visibility_subject: command
                .visibility
                .player_id()
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
            trace_id: context.trace_id().to_string(),
        };
        self.inner
            .lock()
            .map_err(|_| TrpgError::AuditIntegrityViolation)?
            .append(draft)
    }

    pub fn verify(&self) -> KernelResult<Vec<AuditRecord>> {
        self.inner
            .lock()
            .map_err(|_| TrpgError::AuditIntegrityViolation)?
            .verify()
    }
}

impl AuditSink for FormalCommitAudit {
    fn append(&mut self, draft: AuditRecordDraft) -> KernelResult<AuditRecord> {
        self.inner
            .lock()
            .map_err(|_| TrpgError::AuditIntegrityViolation)?
            .append(draft)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormalAuthorization {
    contract: AuthorityContract,
    canonical_audit: CanonicalPolicyAudit,
}

impl FormalAuthorization {
    pub fn contract(&self) -> &AuthorityContract {
        &self.contract
    }

    pub fn canonical_audit(&self) -> &CanonicalPolicyAudit {
        &self.canonical_audit
    }
}

/// Store-owned authorization capability. The canonical identity registry,
/// policy endpoints, and audit custody are fixed when the store is composed;
/// a commit caller supplies only authenticated credentials and a command.
#[derive(Clone)]
pub struct FormalCommitAuthorizer {
    identity_verifier: IdentityVerifier,
    policy: OpenFgaOpaPolicyAdapter,
    audit: FormalCommitAudit,
}

impl std::fmt::Debug for FormalCommitAuthorizer {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FormalCommitAuthorizer")
            .field("identity_verifier", &"[CANONICAL IDENTITY STATE]")
            .field("policy", &"[OPENFGA + OPA]")
            .field("audit", &self.audit)
            .finish()
    }
}

impl FormalCommitAuthorizer {
    pub fn new(
        identity_verifier: IdentityVerifier,
        policy: OpenFgaOpaPolicyAdapter,
        audit: FormalCommitAudit,
    ) -> Self {
        Self {
            identity_verifier,
            policy,
            audit,
        }
    }

    pub fn authorize<T>(
        &self,
        workflow_authentication: &AuthenticationContext,
        authorizing_authentication: Option<&AuthenticationContext>,
        command: &CommandEnvelope<T>,
        requested_role: &str,
        now_unix_ms: u64,
    ) -> KernelResult<FormalAuthorization> {
        if requested_role.trim().is_empty() {
            return Err(TrpgError::InvalidConfiguration(
                "formal_commit_requested_role_required",
            ));
        }
        let campaign_id = command.authenticated_context().resource().campaign_id();
        let contract = self
            .identity_verifier
            .authority_contract(campaign_id)
            .map_err(|_| TrpgError::AuthorityViolation)?;
        contract.validate_command(command)?;
        self.identity_verifier
            .verify_actor(
                workflow_authentication,
                &command.actor,
                campaign_id,
                now_unix_ms,
            )
            .map_err(|_| TrpgError::InternalIdentityInvalid)?;
        if let Some(authentication) = authorizing_authentication {
            self.identity_verifier
                .verify(authentication, now_unix_ms)
                .map_err(|_| TrpgError::InternalIdentityInvalid)?;
            authentication
                .require_campaign(campaign_id)
                .map_err(|_| TrpgError::CampaignScopeMismatch)?;
        }
        let audit_authentication = authorizing_authentication.unwrap_or(workflow_authentication);

        let principal_role = formal_principal_role(command.actor.role())?;
        let request = PolicyAuthorizationRequest {
            actor_id: command.actor.id().to_string(),
            principal_role: principal_role.to_owned(),
            campaign_id: campaign_id.to_string(),
            resource_type: command
                .authenticated_context()
                .resource()
                .resource_type()
                .to_string(),
            resource_id: command
                .authenticated_context()
                .resource()
                .resource_id()
                .to_string(),
            action: "write_official_state".to_owned(),
            authority_mode: authority_mode_name(&command.authority_mode).to_owned(),
            requested_role: None,
            target_visibility: visibility_name(command.visibility.label()).to_owned(),
            target_visibility_subject: command.visibility.player_id().map(ToString::to_string),
            trace_id: command.authenticated_context().trace_id().to_string(),
        };

        let evidence = match self.policy.evaluate(&request) {
            Ok(evidence) => evidence,
            Err(error) => {
                let (openfga_revision, opa_revision) = self.policy.revision_snapshot();
                self.audit.record_policy_decision(
                    audit_authentication,
                    command,
                    &request,
                    requested_role,
                    AuditDecision::Unavailable,
                    "policy-unavailable",
                    openfga_revision,
                    "policy-unavailable",
                    opa_revision,
                )?;
                return Err(error);
            }
        };
        if let Err(error) = evidence.validate() {
            let (openfga_revision, opa_revision) = self.policy.revision_snapshot();
            self.audit.record_policy_decision(
                audit_authentication,
                command,
                &request,
                requested_role,
                AuditDecision::Unavailable,
                "policy-evidence-untrusted",
                openfga_revision,
                "policy-evidence-untrusted",
                opa_revision,
            )?;
            return Err(error);
        }
        self.record_evidence(
            audit_authentication,
            command,
            &request,
            requested_role,
            &evidence,
        )?;
        if !evidence.openfga.allowed || !evidence.opa.allowed {
            return Err(TrpgError::PolicyDenied);
        }
        let (actor_origin, authentication_reference) = audit_actor(audit_authentication);
        Ok(FormalAuthorization {
            canonical_audit: CanonicalPolicyAudit {
                actor_id: audit_authentication.subject_id().to_string(),
                actor_origin: actor_origin.to_owned(),
                authentication_reference: authentication_reference.to_owned(),
                resource_type: request.resource_type.clone(),
                resource_id: request.resource_id.clone(),
                action: request.action.clone(),
                requested_role: requested_role.to_owned(),
                openfga_decision_id: evidence.openfga.decision_id.clone(),
                openfga_policy_revision: evidence.openfga.policy_revision.clone(),
                opa_decision_id: evidence.opa.decision_id.clone(),
                opa_policy_revision: evidence.opa.policy_revision.clone(),
            },
            contract,
        })
    }

    fn record_evidence<T>(
        &self,
        authentication: &AuthenticationContext,
        command: &CommandEnvelope<T>,
        request: &PolicyAuthorizationRequest,
        requested_role: &str,
        evidence: &PolicyEvidence,
    ) -> KernelResult<AuditRecord> {
        self.audit.record_policy_decision(
            authentication,
            command,
            request,
            requested_role,
            if evidence.openfga.allowed && evidence.opa.allowed {
                AuditDecision::Permit
            } else {
                AuditDecision::Deny
            },
            &evidence.openfga.decision_id,
            &evidence.openfga.policy_revision,
            &evidence.opa.decision_id,
            &evidence.opa.policy_revision,
        )
    }
}

fn audit_actor(authentication: &AuthenticationContext) -> (&'static str, &str) {
    match authentication.kind() {
        PrincipalKind::UserSession { session_id, .. } => ("user_session", session_id.as_str()),
        PrincipalKind::Workload { .. } => ("workload", authentication.subject_id().as_str()),
        PrincipalKind::AgentRun { run_id, .. } => ("agent_run", run_id.as_str()),
    }
}

fn formal_principal_role(role: &ActorRole) -> KernelResult<&'static str> {
    match role {
        ActorRole::Workflow => Ok("workflow"),
        ActorRole::RulesEngine => Ok("rules_engine"),
        ActorRole::System => Ok("system"),
        _ => Err(TrpgError::AuthorityViolation),
    }
}

fn authority_mode_name(mode: &AuthorityMode) -> &'static str {
    match mode {
        AuthorityMode::HumanKp => "human_kp",
        AuthorityMode::AiKp => "ai_kp",
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

fn provenance_kind_name(kind: &trpg_shared_kernel::ProvenanceKind) -> &'static str {
    match kind {
        trpg_shared_kernel::ProvenanceKind::HumanKeeperStatement => "human_keeper_statement",
        trpg_shared_kernel::ProvenanceKind::RulesEngineDecision => "rules_engine_decision",
        trpg_shared_kernel::ProvenanceKind::ToolResult => "tool_result",
        trpg_shared_kernel::ProvenanceKind::AgentProposal => "agent_proposal",
        trpg_shared_kernel::ProvenanceKind::ImportedSource => "imported_source",
        trpg_shared_kernel::ProvenanceKind::SystemFixture => "system_fixture",
    }
}
