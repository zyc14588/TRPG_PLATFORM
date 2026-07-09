crate::define_ops_runbook_module!(
    ProjectionRebuildRunbookCommand,
    ProjectionRebuildRunbookService,
    ProjectionRebuildRunbookRepository,
    ProjectionRebuildRunbookError,
    append_projection_rebuild_runbook_event,
    "CODEX-0100-11-OPS-MIGRATION-fee4c9b6ba",
    "projection_rebuild_runbook",
    "OpsProjectionRebuildRunbookRecorded",
    crate::OpsRunbookOperation::ProjectionRebuild,
    ["projection_checkpoint", "projection_hash", "rebuild_audit"],
    "evidence/batches/BATCH-042/projection-rebuild-runbook.md"
);
