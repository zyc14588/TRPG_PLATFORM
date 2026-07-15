use trpg_platform::reliability_performance_impl::{
    record_reliability_performance_guard, RecordReliabilityPerformanceGuard,
    ReliabilityPerformanceEvent, ReliabilityPerformanceRepository,
    RELIABILITY_PERFORMANCE_GUARD_RECORDED_EVENT, RELIABILITY_PERFORMANCE_IMPL_METRIC_MODULE,
};
use trpg_shared_kernel::{
    ActorRole, AuthorityMode, CommandEnvelope, PrincipalScope, TrpgError, Visibility,
    VisibilityLabel,
};

fn command() -> CommandEnvelope<RecordReliabilityPerformanceGuard> {
    trpg_test_support::governed_command(
        RecordReliabilityPerformanceGuard {
            operation: "projection_rebuild".to_owned(),
            attempt: 8,
            base_delay_ms: 100,
            max_delay_ms: 1_000,
            projection_lag_events: 3,
        },
        ActorRole::System,
        AuthorityMode::HumanKp,
    )
}

#[test]
fn reliability_performance_impl_rejects_authority_contract_violation() {
    let command = trpg_test_support::governed_command(
        command().payload,
        ActorRole::AiKeeper,
        AuthorityMode::HumanKp,
    );
    let mut repository = ReliabilityPerformanceRepository::default();

    let err = record_reliability_performance_guard(&mut repository, &command)
        .expect_err("authority mismatch denied");

    assert_eq!(err, TrpgError::AuthorityViolation);
    assert!(repository.events().is_empty());
}

#[test]
fn reliability_performance_impl_keeps_visibility_and_fact_provenance_on_replay() {
    let mut command = command();
    command.visibility = Visibility::new(VisibilityLabel::SystemPrivate);
    let mut repository = ReliabilityPerformanceRepository::default();

    let event =
        record_reliability_performance_guard(&mut repository, &command).expect("guard recorded");

    assert_eq!(
        event.event_type,
        RELIABILITY_PERFORMANCE_GUARD_RECORDED_EVENT
    );
    assert_eq!(event.fact_provenance, command.fact_provenance);
    assert!(repository
        .replay_visible(&PrincipalScope::Public)
        .is_empty());
    assert_eq!(repository.replay_visible(&PrincipalScope::System).len(), 1);
    assert!(matches!(
        event.payload,
        ReliabilityPerformanceEvent::ReliabilityPerformanceGuardRecorded {
            retry_after_ms: 1_000,
            projection_lag_events: 3,
            ..
        }
    ));
}

#[test]
fn reliability_performance_impl_uses_current_safe_metric_module() {
    assert_eq!(
        RELIABILITY_PERFORMANCE_IMPL_METRIC_MODULE,
        "reliability_performance_impl"
    );
}
