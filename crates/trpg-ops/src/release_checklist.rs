crate::define_ops_runbook_module!(
    ReleaseChecklistCommand,
    ReleaseChecklistService,
    ReleaseChecklistRepository,
    ReleaseChecklistError,
    append_release_checklist_event,
    "CODEX-0101-11-OPS-MIGRATION-57b0f58ae0",
    "release_checklist",
    "OpsReleaseChecklistRecorded",
    crate::OpsRunbookOperation::ReleaseChecklist,
    ["release_gate", "rollback_plan", "v1_acceptance_evidence"],
    "evidence/batches/BATCH-042/release-checklist.md"
);
