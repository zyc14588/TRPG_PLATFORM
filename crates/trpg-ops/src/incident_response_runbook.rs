crate::define_ops_runbook_module!(
    IncidentResponseRunbookCommand,
    append_incident_response_runbook_event,
    "incident_response_runbook",
    "OpsIncidentResponseRunbookRecorded",
    crate::OpsRunbookOperation::IncidentResponse,
    ["incident_timeline", "audit_log", "privacy_review"],
    "evidence/ops/incident-response-runbook.md"
);
