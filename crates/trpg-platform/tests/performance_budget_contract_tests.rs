use trpg_platform::performance_budget::{
    ensure_within_budget, record_performance_budget, EvaluatePerformanceBudget, PerformanceBudget,
    PERFORMANCE_BUDGET_EVALUATED_EVENT,
};
use trpg_platform::PlatformEventStore;
use trpg_shared_kernel::{ActorRole, AuthorityMode, CommandEnvelope, TrpgError};

#[test]
fn performance_sample_over_budget_fails_closed() {
    let sample = EvaluatePerformanceBudget {
        budget: PerformanceBudget {
            budget_name: "api_health_latency".to_owned(),
            limit_ms: 250,
        },
        actual_ms: 251,
    };

    let err = ensure_within_budget(&sample).expect_err("over budget denied");

    assert_eq!(err, TrpgError::PolicyDenied);
}

#[test]
fn performance_budget_event_is_recorded_under_limit() {
    let command = CommandEnvelope::governed(
        EvaluatePerformanceBudget {
            budget: PerformanceBudget {
                budget_name: "api_health_latency".to_owned(),
                limit_ms: 250,
            },
            actual_ms: 120,
        },
        ActorRole::System,
        AuthorityMode::HumanKp,
    );
    let mut store = PlatformEventStore::default();

    let event = record_performance_budget(&mut store, &command).expect("budget sample recorded");

    assert_eq!(event.event_type, PERFORMANCE_BUDGET_EVALUATED_EVENT);
}
