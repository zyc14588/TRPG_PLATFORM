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
            campaign_id: request.campaign_id.clone(),
            resource_type: request.resource_type.clone(),
            resource_id: request.resource_id.clone(),
            action: request.action.clone(),
            requested_role: requested_role.to_owned(),
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
