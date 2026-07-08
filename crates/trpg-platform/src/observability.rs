use crate::readme::{
    append_platform_event, redact_for_observability, PlatformEvent, PlatformEventEnvelope,
    PlatformEventStore,
};
use trpg_shared_kernel::{CommandEnvelope, KernelResult, TrpgError};

pub const METRIC_RECORDED_EVENT: &str = "platform.observability.metric_recorded";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordMetric {
    pub metric_name: String,
    pub value: u64,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObservedMetric {
    pub metric_name: String,
    pub value: u64,
    pub detail: String,
}

pub fn observed_metric(command: &CommandEnvelope<RecordMetric>) -> KernelResult<ObservedMetric> {
    if !command.payload.metric_name.starts_with("trpg_platform_") {
        return Err(TrpgError::InvalidConfiguration(
            "platform_metric_name_required",
        ));
    }

    Ok(ObservedMetric {
        metric_name: command.payload.metric_name.clone(),
        value: command.payload.value,
        detail: redact_for_observability(&command.visibility, &command.payload.detail),
    })
}

pub fn record_metric(
    store: &mut PlatformEventStore,
    command: &CommandEnvelope<RecordMetric>,
) -> KernelResult<PlatformEventEnvelope> {
    let metric = observed_metric(command)?;

    append_platform_event(
        store,
        command,
        METRIC_RECORDED_EVENT,
        PlatformEvent::MetricRecorded {
            metric_name: metric.metric_name,
            value: metric.value,
            detail: metric.detail,
        },
    )
}
