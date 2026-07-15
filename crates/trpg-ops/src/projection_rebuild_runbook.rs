crate::define_ops_runbook_module!(
    ProjectionRebuildRunbookCommand,
    ProjectionRebuildRunbookService,
    ProjectionRebuildRunbookRepository,
    ProjectionRebuildRunbookError,
    append_projection_rebuild_runbook_event,
    "projection_rebuild_runbook",
    "OpsProjectionRebuildRunbookRecorded",
    crate::OpsRunbookOperation::ProjectionRebuild,
    ["projection_checkpoint", "projection_hash", "rebuild_audit"],
    "runbooks/projection-rebuild"
);
