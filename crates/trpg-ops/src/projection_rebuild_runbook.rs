crate::define_ops_runbook_module!(
    ProjectionRebuildRunbookCommand,
    append_projection_rebuild_runbook_event,
    "projection_rebuild_runbook",
    "OpsProjectionRebuildRunbookRecorded",
    crate::OpsRunbookOperation::ProjectionRebuild,
    ["projection_checkpoint", "projection_hash", "rebuild_audit"],
    "evidence/ops/projection-rebuild-runbook.md"
);
