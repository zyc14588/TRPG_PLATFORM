use trpg_platform::observability::{
    observed_metric, record_metric, RecordMetric, METRIC_RECORDED_EVENT,
};
use trpg_platform::{PlatformEvent, PlatformEventStore};
use trpg_shared_kernel::{ActorRole, AuthorityMode, TrpgError, Visibility, VisibilityLabel};

#[test]
fn metric_detail_is_redacted_for_restricted_visibility() {
    let mut command = trpg_test_support::governed_command!(
        RecordMetric {
            metric_name: "trpg_platform_worker_lag_ms".to_owned(),
            value: 42,
            detail: "keeper_only_scene_fact".to_owned(),
        },
        ActorRole::System,
        AuthorityMode::HumanKp,
    );
    command.visibility = Visibility::new(VisibilityLabel::KeeperOnly);

    let metric = observed_metric(&command).expect("metric built");

    assert_eq!(metric.detail, "[redacted]");
}

#[test]
fn previous_metric_names_are_not_accepted() {
    let command = trpg_test_support::governed_command!(
        RecordMetric {
            metric_name: "worker_lag_ms".to_owned(),
            value: 1,
            detail: "public".to_owned(),
        },
        ActorRole::System,
        AuthorityMode::HumanKp,
    );

    let err = observed_metric(&command).expect_err("metric name denied");

    assert_eq!(
        err,
        TrpgError::InvalidConfiguration("platform_metric_name_required")
    );
}

#[test]
fn metric_recording_appends_event() {
    let command = trpg_test_support::governed_command!(
        RecordMetric {
            metric_name: "trpg_platform_health_state".to_owned(),
            value: 1,
            detail: "ok".to_owned(),
        },
        ActorRole::System,
        AuthorityMode::HumanKp,
    );
    let mut store = PlatformEventStore::default();

    let event = record_metric(&mut store, &command).expect("metric recorded");

    assert_eq!(event.event_type, METRIC_RECORDED_EVENT);
    assert!(matches!(
        event.payload,
        PlatformEvent::MetricRecorded { .. }
    ));
}
