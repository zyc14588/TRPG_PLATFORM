use crate::readme::{
    append_platform_event, redact_for_observability, PlatformEvent, PlatformEventEnvelope,
    PlatformEventStore,
};
use trpg_shared_kernel::{CommandEnvelope, KernelResult, TrpgError};

pub const AUDIT_TRACE_RECORDED_EVENT: &str = "platform.observability_audit_trace.recorded";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordAuditTrace {
    pub action: String,
    pub detail: String,
}

pub fn record_audit_trace(
    store: &mut PlatformEventStore,
    command: &CommandEnvelope<RecordAuditTrace>,
) -> KernelResult<PlatformEventEnvelope> {
    if command.payload.action.trim().is_empty() {
        return Err(TrpgError::InvalidConfiguration("audit_action_required"));
    }

    append_platform_event(
        store,
        command,
        AUDIT_TRACE_RECORDED_EVENT,
        PlatformEvent::AuditTraceRecorded {
            action: command.payload.action.clone(),
            detail: redact_for_observability(&command.visibility, &command.payload.detail),
        },
    )
}
