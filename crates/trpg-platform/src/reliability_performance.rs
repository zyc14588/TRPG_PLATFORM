use crate::readme::{
    append_platform_event, PlatformEvent, PlatformEventEnvelope, PlatformEventStore,
};
use trpg_shared_kernel::{CommandEnvelope, KernelResult};

pub const RELIABILITY_POLICY_EVALUATED_EVENT: &str =
    "platform.reliability_performance.policy_evaluated";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvaluateReliabilityPolicy {
    pub operation: String,
    pub attempt: u32,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
}

pub fn retry_delay_ms(policy: &EvaluateReliabilityPolicy) -> u64 {
    let shift = policy.attempt.min(16);
    let multiplier = 1_u64.checked_shl(shift).unwrap_or(u64::MAX);
    policy
        .base_delay_ms
        .saturating_mul(multiplier)
        .min(policy.max_delay_ms)
}

pub fn record_reliability_policy(
    store: &mut PlatformEventStore,
    command: &CommandEnvelope<EvaluateReliabilityPolicy>,
) -> KernelResult<PlatformEventEnvelope> {
    let retry_after_ms = retry_delay_ms(&command.payload);

    append_platform_event(
        store,
        command,
        RELIABILITY_POLICY_EVALUATED_EVENT,
        PlatformEvent::ReliabilityPolicyEvaluated {
            operation: command.payload.operation.clone(),
            retry_after_ms,
        },
    )
}
