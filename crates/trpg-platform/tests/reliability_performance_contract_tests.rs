use trpg_platform::reliability_performance::{
    record_reliability_policy, retry_delay_ms, EvaluateReliabilityPolicy,
    RELIABILITY_POLICY_EVALUATED_EVENT,
};
use trpg_platform::PlatformEventStore;
use trpg_shared_kernel::{ActorRole, AuthorityMode};

#[test]
fn retry_delay_is_capped_by_policy() {
    let policy = EvaluateReliabilityPolicy {
        operation: "projection_rebuild".to_owned(),
        attempt: 8,
        base_delay_ms: 100,
        max_delay_ms: 1_000,
    };

    assert_eq!(retry_delay_ms(&policy), 1_000);
}

#[test]
fn reliability_policy_evaluation_is_evented() {
    let command = trpg_test_support::governed_command(
        EvaluateReliabilityPolicy {
            operation: "background_worker_restart".to_owned(),
            attempt: 1,
            base_delay_ms: 100,
            max_delay_ms: 1_000,
        },
        ActorRole::System,
        AuthorityMode::HumanKp,
    );
    let mut store = PlatformEventStore::default();

    let event =
        record_reliability_policy(&mut store, &command).expect("reliability event recorded");

    assert_eq!(event.event_type, RELIABILITY_POLICY_EVALUATED_EVENT);
}
