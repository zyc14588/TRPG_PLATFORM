crate::define_ops_runbook_module!(
    IncidentResponseRunbookCommand,
    IncidentResponseRunbookService,
    IncidentResponseRunbookRepository,
    IncidentResponseRunbookError,
    append_incident_response_runbook_event,
    "CODEX-0098-11-OPS-MIGRATION-feb9c54dda",
    "incident_response_runbook",
    "OpsIncidentResponseRunbookRecorded",
    crate::OpsRunbookOperation::IncidentResponse,
    ["incident_timeline", "audit_log", "privacy_review"],
    "evidence/batches/BATCH-042/incident-response-runbook.md"
);
