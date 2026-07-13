crate::define_ops_runbook_module!(
    BacklogCommand,
    append_backlog_event,
    "backlog",
    "OpsBacklogRecorded",
    crate::OpsRunbookOperation::BacklogReview,
    ["open_question", "risk_register", "handoff_note"],
    "evidence/ops/backlog.md"
);
