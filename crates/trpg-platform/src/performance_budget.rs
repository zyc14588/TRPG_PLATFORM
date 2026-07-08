use crate::readme::{
    append_platform_event, PlatformEvent, PlatformEventEnvelope, PlatformEventStore,
};
use trpg_shared_kernel::{CommandEnvelope, KernelResult, TrpgError};

pub const PERFORMANCE_BUDGET_EVALUATED_EVENT: &str = "platform.performance_budget.evaluated";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PerformanceBudget {
    pub budget_name: String,
    pub limit_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EvaluatePerformanceBudget {
    pub budget: PerformanceBudget,
    pub actual_ms: u64,
}

pub fn ensure_within_budget(sample: &EvaluatePerformanceBudget) -> KernelResult<()> {
    if sample.actual_ms > sample.budget.limit_ms {
        return Err(TrpgError::PolicyDenied);
    }

    Ok(())
}

pub fn record_performance_budget(
    store: &mut PlatformEventStore,
    command: &CommandEnvelope<EvaluatePerformanceBudget>,
) -> KernelResult<PlatformEventEnvelope> {
    ensure_within_budget(&command.payload)?;

    append_platform_event(
        store,
        command,
        PERFORMANCE_BUDGET_EVALUATED_EVENT,
        PlatformEvent::PerformanceBudgetEvaluated {
            budget_name: command.payload.budget.budget_name.clone(),
            actual_ms: command.payload.actual_ms,
            limit_ms: command.payload.budget.limit_ms,
        },
    )
}
