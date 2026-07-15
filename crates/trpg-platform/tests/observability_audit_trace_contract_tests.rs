use trpg_platform::observability_audit_trace::{
    record_audit_trace, RecordAuditTrace, AUDIT_TRACE_RECORDED_EVENT,
};
use trpg_platform::{PlatformEvent, PlatformEventStore};
use trpg_shared_kernel::{ActorRole, AuthorityMode, TrpgError, Visibility, VisibilityLabel};

#[test]
fn audit_trace_redacts_restricted_detail() {
    let mut command = trpg_test_support::governed_command(
        RecordAuditTrace {
            action: "inspect_health".to_owned(),
            detail: "system_private_value".to_owned(),
        },
        ActorRole::System,
        AuthorityMode::HumanKp,
    );
    command.visibility = Visibility::new(VisibilityLabel::SystemPrivate);
    let mut store = PlatformEventStore::default();

    let event = record_audit_trace(&mut store, &command).expect("audit trace recorded");

    assert_eq!(event.event_type, AUDIT_TRACE_RECORDED_EVENT);
    assert!(matches!(
        event.payload,
        PlatformEvent::AuditTraceRecorded { detail, .. } if detail == "[redacted]"
    ));
}

#[test]
fn audit_trace_requires_idempotency_key() {
    let mut command = trpg_test_support::governed_command(
        RecordAuditTrace {
            action: "inspect_health".to_owned(),
            detail: "ok".to_owned(),
        },
        ActorRole::System,
        AuthorityMode::HumanKp,
    );
    command.idempotency_key.clear();
    let mut store = PlatformEventStore::default();

    let err = record_audit_trace(&mut store, &command).expect_err("idempotency required");

    assert_eq!(err, TrpgError::MissingIdempotencyKey);
}
