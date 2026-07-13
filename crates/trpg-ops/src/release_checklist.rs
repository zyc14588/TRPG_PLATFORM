crate::define_ops_runbook_module!(
    ReleaseChecklistCommand,
    append_release_checklist_event,
    "release_checklist",
    "OpsReleaseChecklistRecorded",
    crate::OpsRunbookOperation::ReleaseChecklist,
    ["release_gate", "rollback_plan", "v1_acceptance_evidence"],
    "evidence/ops/release-checklist.md"
);
