pub const MODULE_NAME: &str = "upgrade_rollback";
pub const EVENT_TYPE: &str = "OpsUpgradeRollbackRecorded";
pub const READ_MODELS: &[&str] = &[
    "backup_manifest",
    "rollback_plan",
    "event_store_hash",
    "restore_verification",
];
pub const RUNBOOK_PATH: &str = "runbooks/upgrade-rollback";
pub const OPENAPI_OPERATION_ID: &str = "ops_upgrade_rollback_record";
pub const EVENT_SCHEMA_NAME: &str = "trpg.ops.upgrade_rollback.event_schema";
pub const NATS_SUBJECT: &str = "trpg.ops.upgrade_rollback.recorded";
pub const SQLX_TRANSACTION_BOUNDARY: &str = "sqlx_event_store_transaction_boundary";
pub const EVENT_STORE_APPEND_BOUNDARY: &str = "event_store_append_only";
pub const OPENFGA_RELATION: &str = "ops_migration_operator";
pub const OPA_POLICY: &str = "ops_migration_upgrade_rollback_policy";
pub const TRACING_SPAN: &str = "ops.upgrade_rollback.record";
pub const METRIC_NAME: &str = "trpg_ops_upgrade_rollback_total";
pub const AUDIT_ACTION: &str = "ops_upgrade_rollback_recorded";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpgradeRollbackCommand {
    pub operation: crate::OpsRunbookOperation,
    pub reason: &'static str,
    pub evidence_path: &'static str,
}

impl UpgradeRollbackCommand {
    pub const fn record(reason: &'static str) -> Self {
        Self {
            operation: crate::OpsRunbookOperation::UpgradeRollback,
            reason,
            evidence_path: RUNBOOK_PATH,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpgradeRollbackPolicyGate {
    pub tool_permission_granted: bool,
    pub openfga_allowed: bool,
    pub opa_allowed: bool,
}

impl UpgradeRollbackPolicyGate {
    pub const fn allow() -> Self {
        Self {
            tool_permission_granted: true,
            openfga_allowed: true,
            opa_allowed: true,
        }
    }

    pub const fn deny_tool_permission() -> Self {
        Self {
            tool_permission_granted: false,
            ..Self::allow()
        }
    }

    pub const fn deny_openfga() -> Self {
        Self {
            openfga_allowed: false,
            ..Self::allow()
        }
    }

    pub const fn deny_opa() -> Self {
        Self {
            opa_allowed: false,
            ..Self::allow()
        }
    }

    pub fn authorize(&self) -> crate::KernelResult<()> {
        if self.tool_permission_granted && self.openfga_allowed && self.opa_allowed {
            Ok(())
        } else {
            Err(crate::TrpgError::PolicyDenied)
        }
    }
}

impl Default for UpgradeRollbackPolicyGate {
    fn default() -> Self {
        Self::allow()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpgradeRollbackRepository {
    pub sqlx_transaction_boundary: &'static str,
    pub event_store_append_boundary: &'static str,
}

impl Default for UpgradeRollbackRepository {
    fn default() -> Self {
        Self {
            sqlx_transaction_boundary: SQLX_TRANSACTION_BOUNDARY,
            event_store_append_boundary: EVENT_STORE_APPEND_BOUNDARY,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpgradeRollbackService {
    pub policy_gate: UpgradeRollbackPolicyGate,
    pub repository: UpgradeRollbackRepository,
}

impl UpgradeRollbackService {
    pub const fn new(policy_gate: UpgradeRollbackPolicyGate) -> Self {
        Self {
            policy_gate,
            repository: UpgradeRollbackRepository {
                sqlx_transaction_boundary: SQLX_TRANSACTION_BOUNDARY,
                event_store_append_boundary: EVENT_STORE_APPEND_BOUNDARY,
            },
        }
    }

    pub fn execute(
        &self,
        store: &mut crate::OpsEventStore,
        authority: &crate::AuthorityContract,
        command: &crate::CommandEnvelope<UpgradeRollbackCommand>,
    ) -> crate::KernelResult<UpgradeRollbackExecution> {
        self.policy_gate.authorize()?;
        let event = append_upgrade_rollback_event(store, authority, command)?;

        Ok(UpgradeRollbackExecution {
            transaction: UpgradeRollbackTransactionEvidence {
                sqlx_boundary: self.repository.sqlx_transaction_boundary,
                event_store_boundary: self.repository.event_store_append_boundary,
                expected_version: command.expected_version,
                event_sequence: event.sequence,
            },
            external_contract: UpgradeRollbackExternalContract::current(),
            observability: UpgradeRollbackObservabilityRecord::from_command(command),
            event,
        })
    }
}

impl Default for UpgradeRollbackService {
    fn default() -> Self {
        Self::new(UpgradeRollbackPolicyGate::allow())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpgradeRollbackError {
    GovernanceViolation(&'static str),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpgradeRollbackTransactionEvidence {
    pub sqlx_boundary: &'static str,
    pub event_store_boundary: &'static str,
    pub expected_version: u64,
    pub event_sequence: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpgradeRollbackExternalContract {
    pub openapi_operation_id: &'static str,
    pub event_schema_name: &'static str,
    pub nats_subject: &'static str,
    pub event_type: &'static str,
    pub openfga_relation: &'static str,
    pub opa_policy: &'static str,
}

impl UpgradeRollbackExternalContract {
    pub const fn current() -> Self {
        Self {
            openapi_operation_id: OPENAPI_OPERATION_ID,
            event_schema_name: EVENT_SCHEMA_NAME,
            nats_subject: NATS_SUBJECT,
            event_type: EVENT_TYPE,
            openfga_relation: OPENFGA_RELATION,
            opa_policy: OPA_POLICY,
        }
    }

    pub fn uses_current_safe_names(&self) -> bool {
        [
            self.openapi_operation_id,
            self.event_schema_name,
            self.nats_subject,
            self.event_type,
            self.openfga_relation,
            self.opa_policy,
        ]
        .into_iter()
        .all(crate::is_current_safe_name)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpgradeRollbackObservabilityRecord {
    pub tracing_span: &'static str,
    pub metric_name: &'static str,
    pub audit_action: &'static str,
    pub correlation_id: String,
    pub causation_id: String,
}

impl UpgradeRollbackObservabilityRecord {
    pub fn from_command(command: &crate::CommandEnvelope<UpgradeRollbackCommand>) -> Self {
        Self {
            tracing_span: TRACING_SPAN,
            metric_name: METRIC_NAME,
            audit_action: AUDIT_ACTION,
            correlation_id: command.correlation_id.as_str().to_owned(),
            causation_id: command.causation_id.as_str().to_owned(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpgradeRollbackExecution {
    pub event: crate::OpsEventEnvelope,
    pub transaction: UpgradeRollbackTransactionEvidence,
    pub external_contract: UpgradeRollbackExternalContract,
    pub observability: UpgradeRollbackObservabilityRecord,
}

pub fn append_upgrade_rollback_event<T>(
    store: &mut crate::OpsEventStore,
    authority: &crate::AuthorityContract,
    command: &crate::CommandEnvelope<T>,
) -> crate::KernelResult<crate::OpsEventEnvelope> {
    crate::append_ops_event(store, authority, command, contract(), RUNBOOK_PATH)
}

pub fn contract() -> crate::OpsRunbookContract {
    crate::OpsRunbookContract::new(
        MODULE_NAME,
        EVENT_TYPE,
        crate::OpsRunbookOperation::UpgradeRollback,
        READ_MODELS,
    )
}
