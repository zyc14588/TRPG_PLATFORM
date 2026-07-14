pub const MODULE_NAME: &str = "upgrade_rollback_impl";
pub const EVENT_TYPE: &str = "OpsUpgradeRollbackImplRecorded";
pub const READ_MODELS: &[&str] = &[
    "migration_ledger",
    "rollback_plan",
    "event_store_hash",
    "projection_replay",
];
pub const RUNBOOK_PATH: &str = "runbooks/upgrade-rollback-implementation";
pub const OPENAPI_OPERATION_ID: &str = "ops_upgrade_rollback_impl_record";
pub const EVENT_SCHEMA_NAME: &str = "trpg.ops.upgrade_rollback_impl.event_schema";
pub const NATS_SUBJECT: &str = "trpg.ops.upgrade_rollback_impl.recorded";
pub const SQLX_TRANSACTION_BOUNDARY: &str = "sqlx_event_store_transaction_boundary";
pub const EVENT_STORE_APPEND_BOUNDARY: &str = "event_store_append_only";
pub const OPENFGA_RELATION: &str = "ops_migration_operator";
pub const OPA_POLICY: &str = "ops_migration_upgrade_rollback_impl_policy";
pub const TRACING_SPAN: &str = "ops.upgrade_rollback_impl.record";
pub const METRIC_NAME: &str = "trpg_ops_upgrade_rollback_impl_total";
pub const AUDIT_ACTION: &str = "ops_upgrade_rollback_impl_recorded";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpgradeRollbackImplCommand {
    pub operation: crate::OpsRunbookOperation,
    pub reason: &'static str,
    pub evidence_path: &'static str,
}

impl UpgradeRollbackImplCommand {
    pub const fn record(reason: &'static str) -> Self {
        Self {
            operation: crate::OpsRunbookOperation::UpgradeRollbackImpl,
            reason,
            evidence_path: RUNBOOK_PATH,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpgradeRollbackImplPolicyGate {
    pub tool_permission_granted: bool,
    pub openfga_allowed: bool,
    pub opa_allowed: bool,
}

impl UpgradeRollbackImplPolicyGate {
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

impl Default for UpgradeRollbackImplPolicyGate {
    fn default() -> Self {
        Self::allow()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpgradeRollbackImplRepository {
    pub sqlx_transaction_boundary: &'static str,
    pub event_store_append_boundary: &'static str,
}

impl Default for UpgradeRollbackImplRepository {
    fn default() -> Self {
        Self {
            sqlx_transaction_boundary: SQLX_TRANSACTION_BOUNDARY,
            event_store_append_boundary: EVENT_STORE_APPEND_BOUNDARY,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpgradeRollbackImplService {
    pub policy_gate: UpgradeRollbackImplPolicyGate,
    pub repository: UpgradeRollbackImplRepository,
}

impl UpgradeRollbackImplService {
    pub const fn new(policy_gate: UpgradeRollbackImplPolicyGate) -> Self {
        Self {
            policy_gate,
            repository: UpgradeRollbackImplRepository {
                sqlx_transaction_boundary: SQLX_TRANSACTION_BOUNDARY,
                event_store_append_boundary: EVENT_STORE_APPEND_BOUNDARY,
            },
        }
    }

    pub fn execute(
        &self,
        store: &mut crate::OpsEventStore,
        authority: &crate::AuthorityContract,
        command: &crate::CommandEnvelope<UpgradeRollbackImplCommand>,
    ) -> crate::KernelResult<UpgradeRollbackImplExecution> {
        self.policy_gate.authorize()?;
        let event = append_upgrade_rollback_impl_event(store, authority, command)?;

        Ok(UpgradeRollbackImplExecution {
            transaction: UpgradeRollbackImplTransactionEvidence {
                sqlx_boundary: self.repository.sqlx_transaction_boundary,
                event_store_boundary: self.repository.event_store_append_boundary,
                expected_version: command.expected_version,
                event_sequence: event.sequence,
            },
            external_contract: UpgradeRollbackImplExternalContract::current(),
            observability: UpgradeRollbackImplObservabilityRecord::from_command(command),
            event,
        })
    }
}

impl Default for UpgradeRollbackImplService {
    fn default() -> Self {
        Self::new(UpgradeRollbackImplPolicyGate::allow())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UpgradeRollbackImplError {
    GovernanceViolation(&'static str),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpgradeRollbackImplTransactionEvidence {
    pub sqlx_boundary: &'static str,
    pub event_store_boundary: &'static str,
    pub expected_version: u64,
    pub event_sequence: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UpgradeRollbackImplExternalContract {
    pub openapi_operation_id: &'static str,
    pub event_schema_name: &'static str,
    pub nats_subject: &'static str,
    pub event_type: &'static str,
    pub openfga_relation: &'static str,
    pub opa_policy: &'static str,
}

impl UpgradeRollbackImplExternalContract {
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
pub struct UpgradeRollbackImplObservabilityRecord {
    pub tracing_span: &'static str,
    pub metric_name: &'static str,
    pub audit_action: &'static str,
    pub correlation_id: String,
    pub causation_id: String,
}

impl UpgradeRollbackImplObservabilityRecord {
    pub fn from_command(command: &crate::CommandEnvelope<UpgradeRollbackImplCommand>) -> Self {
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
pub struct UpgradeRollbackImplExecution {
    pub event: crate::OpsEventEnvelope,
    pub transaction: UpgradeRollbackImplTransactionEvidence,
    pub external_contract: UpgradeRollbackImplExternalContract,
    pub observability: UpgradeRollbackImplObservabilityRecord,
}

pub fn append_upgrade_rollback_impl_event<T>(
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
        crate::OpsRunbookOperation::UpgradeRollbackImpl,
        READ_MODELS,
    )
}
