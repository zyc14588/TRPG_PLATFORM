crate::define_ops_runbook_module!(
    BacklogCommand,
    BacklogService,
    BacklogRepository,
    BacklogError,
    append_backlog_event,
    "CODEX-0920-11-OPS-MIGRATION-ebe3f221d7",
    "backlog",
    "OpsBacklogRecorded",
    crate::OpsRunbookOperation::BacklogReview,
    ["open_question", "risk_register", "handoff_note"],
    "evidence/batches/BATCH-042/backlog.md"
);
