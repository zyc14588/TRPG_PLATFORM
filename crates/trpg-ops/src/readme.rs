use trpg_shared_kernel::{
    AuthorityContract, CommandEnvelope, EventEnvelope, EventStore, KernelResult, PrincipalScope,
    TrpgError, Visibility, VisibilityLabel,
};

pub const S10_BACKUP_EVENT_HASH: &str =
    "sha256:ea4d9a5f8aa58929b9514a881c876e66557ee9706e82a2e251cdb247c6f4141b";
pub const S10_RESTORE_EVENT_HASH: &str = S10_BACKUP_EVENT_HASH;
pub const S10_PROJECTION_HASH: &str =
    "sha256:15d1765619e2e0b512f21a2a1ccfb56b727efecbeadccbbd919ee88199e747e2";
pub const VISIBILITY_REDACTED: &str = "[redacted]";

pub const OPS_REQUIRED_COMMAND_FIELDS: &[&str] = &[
    "idempotency_key",
    "expected_version",
    "actor",
    "authority_mode",
    "visibility",
    "fact_provenance",
    "correlation_id",
    "causation_id",
];

pub const OPS_METRIC: &str = "trpg_ops_runbook_total";
pub const OPS_NATS_SUBJECT: &str = "trpg.ops.runbook.recorded";
pub const OPS_CANON_BOUNDARY: &str = "command_workflow_decision_event_store_projection";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpsRunbookOperation {
    BackupRestore,
    IncidentResponse,
    MigrationUpgradeRollback,
    ProjectionRebuild,
    ReleaseChecklist,
    ReadmeRecord,
    ImplementationPlan,
    BacklogReview,
    UpgradeBackupReplay,
    UpgradeRollbackImpl,
    UpgradeRollback,
}

impl OpsRunbookOperation {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::BackupRestore => "backup_restore",
            Self::IncidentResponse => "incident_response",
            Self::MigrationUpgradeRollback => "migration_upgrade_rollback",
            Self::ProjectionRebuild => "projection_rebuild",
            Self::ReleaseChecklist => "release_checklist",
            Self::ReadmeRecord => "readme_record",
            Self::ImplementationPlan => "implementation_plan",
            Self::BacklogReview => "backlog_review",
            Self::UpgradeBackupReplay => "upgrade_backup_replay",
            Self::UpgradeRollbackImpl => "upgrade_rollback_impl",
            Self::UpgradeRollback => "upgrade_rollback",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpsRunbookCommand {
    pub operation: OpsRunbookOperation,
    pub reason: &'static str,
    pub evidence_path: &'static str,
}

impl OpsRunbookCommand {
    pub const fn record(
        operation: OpsRunbookOperation,
        reason: &'static str,
        evidence_path: &'static str,
    ) -> Self {
        Self {
            operation,
            reason,
            evidence_path,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackupManifest {
    pub object_key: String,
    pub sha256: String,
    pub created_at: String,
    pub schema_version: String,
}

impl BackupManifest {
    pub fn fixture() -> Self {
        Self {
            object_key: "backups/campaign_001.snapshot".to_owned(),
            sha256: S10_BACKUP_EVENT_HASH.to_owned(),
            created_at: "2026-07-02T00:00:00Z".to_owned(),
            schema_version: "s10.v1".to_owned(),
        }
    }

    pub fn has_required_fields(&self) -> bool {
        !self.object_key.trim().is_empty()
            && self.sha256.starts_with("sha256:")
            && !self.created_at.trim().is_empty()
            && !self.schema_version.trim().is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunbookExecutionRecord {
    pub operator: String,
    pub command: String,
    pub exit_code: i32,
    pub evidence_path: String,
}

impl RunbookExecutionRecord {
    pub fn succeeded(command: impl Into<String>, evidence_path: impl Into<String>) -> Self {
        Self {
            operator: "ops_operator".to_owned(),
            command: command.into(),
            exit_code: 0,
            evidence_path: evidence_path.into(),
        }
    }

    pub fn has_required_fields(&self) -> bool {
        !self.operator.trim().is_empty()
            && !self.command.trim().is_empty()
            && !self.evidence_path.trim().is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpsRunbookEventRecord {
    pub module_name: &'static str,
    pub operation: OpsRunbookOperation,
    pub evidence_path: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpsEvent {
    RunbookStepRecorded(OpsRunbookEventRecord),
    BackupCompleted(BackupManifest),
    RestoreVerified {
        before_hash: String,
        after_hash: String,
    },
    ProjectionRebuildVerified(OpsProjectionReport),
}

pub type OpsEventEnvelope = EventEnvelope<OpsEvent>;
pub type OpsEventStore = EventStore<OpsEvent>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpsRunbookError {
    RestoreHashMismatch,
    RollbackRunbookRequired,
    ProjectionRebuildHashMismatch,
}

impl OpsRunbookError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::RestoreHashMismatch => "RESTORE_HASH_MISMATCH",
            Self::RollbackRunbookRequired => "ROLLBACK_RUNBOOK_REQUIRED",
            Self::ProjectionRebuildHashMismatch => "PROJECTION_REBUILD_HASH_MISMATCH",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpsRunbookContract {
    pub prompt_id: &'static str,
    pub module_name: &'static str,
    pub event_type: &'static str,
    pub operation: OpsRunbookOperation,
    pub read_models: &'static [&'static str],
    pub nats_subject: &'static str,
    pub metric: &'static str,
    pub required_command_fields: &'static [&'static str],
    pub canon_boundary: &'static str,
}

impl OpsRunbookContract {
    pub const fn new(
        prompt_id: &'static str,
        module_name: &'static str,
        event_type: &'static str,
        operation: OpsRunbookOperation,
        read_models: &'static [&'static str],
    ) -> Self {
        Self {
            prompt_id,
            module_name,
            event_type,
            operation,
            read_models,
            nats_subject: OPS_NATS_SUBJECT,
            metric: OPS_METRIC,
            required_command_fields: OPS_REQUIRED_COMMAND_FIELDS,
            canon_boundary: OPS_CANON_BOUNDARY,
        }
    }

    pub fn uses_current_safe_names(&self) -> bool {
        [
            self.module_name,
            self.event_type,
            self.nats_subject,
            self.metric,
            self.canon_boundary,
        ]
        .into_iter()
        .all(is_current_safe_name)
            && self.read_models.iter().copied().all(is_current_safe_name)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OpsProjectionReport {
    pub source_event_count: usize,
    pub new_canon_events: usize,
    pub projection_hash: String,
}

pub fn append_ops_event<T>(
    store: &mut OpsEventStore,
    contract: &AuthorityContract,
    command: &CommandEnvelope<T>,
    runbook: OpsRunbookContract,
    evidence_path: &'static str,
) -> KernelResult<OpsEventEnvelope> {
    if !runbook.uses_current_safe_names() {
        return Err(TrpgError::CodingPolicyViolation(
            "ops_runbook_current_safe_name",
        ));
    }

    contract.validate_command(command)?;

    store.append(
        command,
        runbook.event_type,
        OpsEvent::RunbookStepRecorded(OpsRunbookEventRecord {
            module_name: runbook.module_name,
            operation: runbook.operation,
            evidence_path,
        }),
    )
}

pub fn verify_restore_hash(before_hash: &str, after_hash: &str) -> Result<(), OpsRunbookError> {
    if before_hash == after_hash {
        Ok(())
    } else {
        Err(OpsRunbookError::RestoreHashMismatch)
    }
}

pub fn rebuild_projection_from_ops_events(events: &[OpsEventEnvelope]) -> OpsProjectionReport {
    OpsProjectionReport {
        source_event_count: events.len(),
        new_canon_events: 0,
        projection_hash: stable_projection_hash(events),
    }
}

pub fn verify_projection_rebuild(
    before_event_count: usize,
    after_event_count: usize,
    actual_hash: &str,
    expected_hash: &str,
) -> Result<OpsProjectionReport, OpsRunbookError> {
    if before_event_count != after_event_count || actual_hash != expected_hash {
        return Err(OpsRunbookError::ProjectionRebuildHashMismatch);
    }

    Ok(OpsProjectionReport {
        source_event_count: after_event_count,
        new_canon_events: 0,
        projection_hash: actual_hash.to_owned(),
    })
}

pub fn redact_ops_output(visibility: &Visibility, text: &str) -> String {
    if restricted_visibility(visibility.label()) {
        VISIBILITY_REDACTED.to_owned()
    } else {
        text.to_owned()
    }
}

pub fn replay_visible_ops_events(
    store: &OpsEventStore,
    principal: &PrincipalScope,
) -> Vec<OpsEventEnvelope> {
    store.replay_visible(principal)
}

pub fn all_batch_042_contracts() -> Vec<OpsRunbookContract> {
    vec![
        crate::backup_restore_runbook::contract(),
        crate::incident_response_runbook::contract(),
        crate::migration_upgrade_rollback::contract(),
        crate::projection_rebuild_runbook::contract(),
        crate::release_checklist::contract(),
        contract(),
        crate::implementation_plan::contract(),
        crate::backlog::contract(),
        crate::upgrade_backup_replay_runbooks::contract(),
    ]
}

pub fn all_batch_043_contracts() -> Vec<OpsRunbookContract> {
    vec![
        crate::upgrade_rollback_impl::contract(),
        crate::upgrade_rollback::contract(),
    ]
}

pub fn contract() -> OpsRunbookContract {
    OpsRunbookContract::new(
        "CODEX-0917-11-OPS-MIGRATION-9f27ade2d3",
        "readme",
        "OpsReadmeRecorded",
        OpsRunbookOperation::ReadmeRecord,
        &["ops_runbook_index"],
    )
}

pub fn append_readme_event<T>(
    store: &mut OpsEventStore,
    authority: &AuthorityContract,
    command: &CommandEnvelope<T>,
) -> KernelResult<OpsEventEnvelope> {
    append_ops_event(
        store,
        authority,
        command,
        contract(),
        "docs/codex/11-ops-migration/README.md",
    )
}

pub fn is_current_safe_name(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return false;
    }

    let lower = trimmed.to_ascii_lowercase();
    let denied = [
        "generated-from-source",
        "generated_from_source",
        "source-breakdow",
        "source_breakdow",
        "docs-implementation",
        "docs_implementation",
        "fix-history",
        "fix_history",
        "legacy",
        "v3",
        "v4",
        "v5",
        "v6",
    ];

    if denied.iter().any(|token| lower.contains(token)) {
        return false;
    }

    trimmed
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
        && !has_long_hex_run(trimmed)
}

fn restricted_visibility(label: &VisibilityLabel) -> bool {
    matches!(
        label,
        VisibilityLabel::KeeperOnly
            | VisibilityLabel::PrivateToPlayer
            | VisibilityLabel::InvestigatorPrivate
            | VisibilityLabel::AiInternal
            | VisibilityLabel::SystemOnly
            | VisibilityLabel::SystemPrivate
    )
}

fn has_long_hex_run(value: &str) -> bool {
    let mut run = 0;
    for ch in value.chars() {
        if ch.is_ascii_hexdigit() {
            run += 1;
            if run >= 10 {
                return true;
            }
        } else {
            run = 0;
        }
    }

    false
}

fn stable_projection_hash(events: &[OpsEventEnvelope]) -> String {
    let mut hash = 0xcbf29ce484222325u64;
    for event in events {
        update_hash(&mut hash, event.sequence.to_string().as_bytes());
        update_hash(&mut hash, event.event_type.as_bytes());
        if let OpsEvent::RunbookStepRecorded(record) = &event.payload {
            update_hash(&mut hash, record.module_name.as_bytes());
            update_hash(&mut hash, record.operation.as_str().as_bytes());
        }
    }

    format!("sha256:ops-{hash:016x}")
}

fn update_hash(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x100000001b3);
    }
}

#[macro_export]
macro_rules! define_ops_runbook_module {
    (
        $command:ident,
        $service:ident,
        $repository:ident,
        $error:ident,
        $append_fn:ident,
        $prompt_id:literal,
        $module_name:literal,
        $event_type:literal,
        $operation:expr,
        [$($read_model:literal),* $(,)?],
        $evidence_path:literal
    ) => {
        pub const PROMPT_ID: &str = $prompt_id;
        pub const MODULE_NAME: &str = $module_name;
        pub const EVENT_TYPE: &str = $event_type;
        pub const READ_MODELS: &[&str] = &[$($read_model),*];
        pub const EVIDENCE_PATH: &str = $evidence_path;

        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct $command {
            pub operation: $crate::OpsRunbookOperation,
            pub reason: &'static str,
            pub evidence_path: &'static str,
        }

        impl $command {
            pub const fn record(reason: &'static str) -> Self {
                Self {
                    operation: $operation,
                    reason,
                    evidence_path: EVIDENCE_PATH,
                }
            }
        }

        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
        pub struct $service;

        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
        pub struct $repository;

        #[derive(Clone, Debug, PartialEq, Eq)]
        pub enum $error {
            GovernanceViolation(&'static str),
        }

        pub fn $append_fn<T>(
            store: &mut $crate::OpsEventStore,
            authority: &$crate::AuthorityContract,
            command: &$crate::CommandEnvelope<T>,
        ) -> $crate::KernelResult<$crate::OpsEventEnvelope> {
            $crate::append_ops_event(store, authority, command, contract(), EVIDENCE_PATH)
        }

        pub fn contract() -> $crate::OpsRunbookContract {
            $crate::OpsRunbookContract::new(
                PROMPT_ID,
                MODULE_NAME,
                EVENT_TYPE,
                $operation,
                READ_MODELS,
            )
        }
    };
}
