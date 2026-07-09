pub mod backlog;
pub mod backup_restore_runbook;
pub mod implementation_plan;
pub mod incident_response_runbook;
pub mod migration_upgrade_rollback;
pub mod projection_rebuild_runbook;
pub mod readme;
pub mod release_checklist;
pub mod upgrade_backup_replay_runbooks;

pub use readme::{
    all_batch_042_contracts, append_ops_event, is_current_safe_name,
    rebuild_projection_from_ops_events, redact_ops_output, verify_projection_rebuild,
    verify_restore_hash, BackupManifest, OpsEvent, OpsEventEnvelope, OpsEventStore,
    OpsProjectionReport, OpsRunbookCommand, OpsRunbookContract, OpsRunbookError,
    OpsRunbookEventRecord, OpsRunbookOperation, RunbookExecutionRecord, S10_BACKUP_EVENT_HASH,
    S10_PROJECTION_HASH, S10_RESTORE_EVENT_HASH, VISIBILITY_REDACTED,
};

pub use trpg_shared_kernel::{
    Actor, ActorRole, AuthorityContract, AuthorityMode, CommandEnvelope, EntityId, EventEnvelope,
    EventStore, FactProvenance, FormalWritePath, KernelResult, PrincipalScope, ProvenanceKind,
    TrpgError, Visibility, VisibilityLabel,
};
