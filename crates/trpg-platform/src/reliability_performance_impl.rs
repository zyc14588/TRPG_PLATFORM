use crate::reliability_performance::{retry_delay_ms, EvaluateReliabilityPolicy};
use trpg_shared_kernel::{CommandEnvelope, EventEnvelope, EventStore, KernelResult, TrpgError};

pub const RELIABILITY_PERFORMANCE_GUARD_RECORDED_EVENT: &str =
    "platform.reliability_performance_impl.guard_recorded";
pub const RELIABILITY_PERFORMANCE_IMPL_METRIC_MODULE: &str = "reliability_performance_impl";
pub const RELIABILITY_PERFORMANCE_IMPL_REQUIRED_METRICS: &[&str] = &[
    "trpg_command_total",
    "trpg_event_append_latency_ms",
    "trpg_policy_deny_total",
    "trpg_projection_lag_events",
    "trpg_visibility_redaction_total",
];

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordReliabilityPerformanceGuard {
    pub operation: String,
    pub attempt: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub projection_lag_events: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub enum ReliabilityPerformanceEvent {
    ReliabilityPerformanceGuardRecorded {
        operation: String,
        retry_after_ms: u64,
        projection_lag_events: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReliabilityPerformanceError {
    OperationRequired,
}

impl From<ReliabilityPerformanceError> for TrpgError {
    fn from(error: ReliabilityPerformanceError) -> Self {
        match error {
            ReliabilityPerformanceError::OperationRequired => {
                TrpgError::InvalidConfiguration("operation_required")
            }
        }
    }
}

pub type ReliabilityPerformanceEventEnvelope = EventEnvelope<ReliabilityPerformanceEvent>;
pub type ReliabilityPerformanceRepository = EventStore<ReliabilityPerformanceEvent>;

pub struct ReliabilityPerformanceService;

impl ReliabilityPerformanceService {
    pub fn record_reliability_performance_guard(
        repository: &mut ReliabilityPerformanceRepository,
        command: &CommandEnvelope<RecordReliabilityPerformanceGuard>,
    ) -> KernelResult<ReliabilityPerformanceEventEnvelope> {
        if command.payload.operation.trim().is_empty() {
            return Err(ReliabilityPerformanceError::OperationRequired.into());
        }

        let retry_after_ms = retry_delay_ms(&EvaluateReliabilityPolicy {
            operation: command.payload.operation.clone(),
            attempt: command.payload.attempt,
            base_delay_ms: command.payload.base_delay_ms,
            max_delay_ms: command.payload.max_delay_ms,
        });

        repository.append(
            command,
            RELIABILITY_PERFORMANCE_GUARD_RECORDED_EVENT,
            ReliabilityPerformanceEvent::ReliabilityPerformanceGuardRecorded {
                operation: command.payload.operation.clone(),
                retry_after_ms,
                projection_lag_events: command.payload.projection_lag_events,
            },
        )
    }
}

pub fn record_reliability_performance_guard(
    repository: &mut ReliabilityPerformanceRepository,
    command: &CommandEnvelope<RecordReliabilityPerformanceGuard>,
) -> KernelResult<ReliabilityPerformanceEventEnvelope> {
    ReliabilityPerformanceService::record_reliability_performance_guard(repository, command)
}
