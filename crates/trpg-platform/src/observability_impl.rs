use crate::readme::redact_for_observability;
use trpg_shared_kernel::{CommandEnvelope, EventEnvelope, EventStore, KernelResult, TrpgError};

pub const PLATFORM_OBSERVATION_RECORDED_EVENT: &str =
    "platform.observability_impl.observation_recorded";
pub const OBSERVABILITY_IMPL_METRIC_MODULE: &str = "observability_impl";
pub const OBSERVABILITY_IMPL_REQUIRED_METRICS: &[&str] = &[
    "trpg_command_total",
    "trpg_event_append_latency_ms",
    "trpg_policy_deny_total",
    "trpg_projection_lag_events",
    "trpg_visibility_redaction_total",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordPlatformObservation {
    pub metric_name: String,
    pub value: u64,
    pub detail: String,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub enum ObservabilityEvent {
    PlatformObservationRecorded {
        metric_name: String,
        value: u64,
        detail: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ObservabilityError {
    PlatformMetricNameRequired,
}

impl From<ObservabilityError> for TrpgError {
    fn from(error: ObservabilityError) -> Self {
        match error {
            ObservabilityError::PlatformMetricNameRequired => {
                TrpgError::InvalidConfiguration("platform_metric_name_required")
            }
        }
    }
}

pub type ObservabilityEventEnvelope = EventEnvelope<ObservabilityEvent>;
pub type ObservabilityRepository = EventStore<ObservabilityEvent>;

pub struct ObservabilityService;

impl ObservabilityService {
    pub fn record_platform_observation(
        repository: &mut ObservabilityRepository,
        command: &CommandEnvelope<RecordPlatformObservation>,
    ) -> KernelResult<ObservabilityEventEnvelope> {
        if !command.payload.metric_name.starts_with("trpg_platform_") {
            return Err(ObservabilityError::PlatformMetricNameRequired.into());
        }

        repository.append(
            command,
            PLATFORM_OBSERVATION_RECORDED_EVENT,
            ObservabilityEvent::PlatformObservationRecorded {
                metric_name: command.payload.metric_name.clone(),
                value: command.payload.value,
                detail: redact_for_observability(&command.visibility, &command.payload.detail),
            },
        )
    }
}

pub fn record_platform_observation(
    repository: &mut ObservabilityRepository,
    command: &CommandEnvelope<RecordPlatformObservation>,
) -> KernelResult<ObservabilityEventEnvelope> {
    ObservabilityService::record_platform_observation(repository, command)
}
