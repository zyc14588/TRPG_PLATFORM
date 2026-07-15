use std::path::Path;
use std::sync::{Arc, Mutex};

use trpg_identity::{AuthenticationContext, PrincipalKind};
use trpg_shared_kernel::{
    Actor, ActorOrigin, AuthorityContract, CommandEnvelope, KernelResult, TrpgError,
};

use crate::tamper_evident_audit::{
    AuditDecision, AuditRecord, AuditRecordDraft, AuditSink, FileAuditLog,
};

/// Cloneable capability for the external, tamper-evident audit witness that
/// every formal runtime or agent commit must persist before changing state.
#[derive(Clone)]
pub struct FormalCommitAudit {
    inner: Arc<Mutex<FileAuditLog>>,
}

struct FormalCommitPrincipal<'a> {
    actor_id: &'a str,
    actor_origin: &'a str,
    authentication_reference: &'a str,
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

    pub fn record_authorized_commit<T>(
        &self,
        authentication: &AuthenticationContext,
        command: &CommandEnvelope<T>,
        contract: &AuthorityContract,
        action: &'static str,
        requested_role: &'static str,
    ) -> KernelResult<AuditRecord> {
        let (actor_origin, authentication_reference) = match authentication.kind() {
            PrincipalKind::UserSession { session_id, .. } => ("user_session", session_id.as_str()),
            PrincipalKind::Workload { .. } => ("workload", authentication.subject_id().as_str()),
            PrincipalKind::AgentRun { run_id, .. } => ("agent_run", run_id.as_str()),
        };
        self.record(
            FormalCommitPrincipal {
                actor_id: authentication.subject_id().as_str(),
                actor_origin,
                authentication_reference,
            },
            command,
            contract,
            action,
            requested_role,
        )
    }

    pub fn record_actor_authorized_commit<T>(
        &self,
        actor: &Actor,
        command: &CommandEnvelope<T>,
        contract: &AuthorityContract,
        action: &'static str,
        requested_role: &'static str,
    ) -> KernelResult<AuditRecord> {
        let (actor_origin, authentication_reference) = match actor.origin() {
            ActorOrigin::UserSession { session_id } => ("user_session", session_id.as_str()),
            ActorOrigin::Workload { .. } => ("workload", actor.id().as_str()),
            ActorOrigin::AgentRun { run_id, .. } => ("agent_run", run_id.as_str()),
        };
        self.record(
            FormalCommitPrincipal {
                actor_id: actor.id().as_str(),
                actor_origin,
                authentication_reference,
            },
            command,
            contract,
            action,
            requested_role,
        )
    }

    fn record<T>(
        &self,
        principal: FormalCommitPrincipal<'_>,
        command: &CommandEnvelope<T>,
        contract: &AuthorityContract,
        action: &'static str,
        requested_role: &'static str,
    ) -> KernelResult<AuditRecord> {
        let context = command.authenticated_context();
        let authority_revision = format!(
            "authority-contract/{}/v{}",
            contract.contract_id().as_str(),
            contract.version()
        );
        let draft = AuditRecordDraft {
            actor_id: principal.actor_id.to_owned(),
            actor_origin: principal.actor_origin.to_owned(),
            authentication_reference: principal.authentication_reference.to_owned(),
            campaign_id: context.resource().campaign_id().to_string(),
            resource_type: context.resource().resource_type().to_string(),
            resource_id: context.resource().resource_id().to_string(),
            action: action.to_owned(),
            requested_role: requested_role.to_owned(),
            decision: AuditDecision::Permit,
            openfga_decision_id: "canonical-authority-contract-permit".to_owned(),
            openfga_policy_revision: authority_revision.clone(),
            opa_decision_id: "canonical-authority-contract-permit".to_owned(),
            opa_policy_revision: authority_revision,
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
