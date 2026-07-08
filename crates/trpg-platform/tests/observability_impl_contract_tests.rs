use trpg_platform::observability_impl::{
    record_platform_observation, ObservabilityEvent, ObservabilityRepository,
    RecordPlatformObservation, OBSERVABILITY_IMPL_METRIC_MODULE,
    PLATFORM_OBSERVATION_RECORDED_EVENT,
};
use trpg_shared_kernel::{
    ActorRole, AuthorityMode, CommandEnvelope, PrincipalScope, TrpgError, Visibility,
    VisibilityLabel,
};

fn command() -> CommandEnvelope<RecordPlatformObservation> {
    CommandEnvelope::governed(
        RecordPlatformObservation {
            metric_name: "trpg_platform_command_total".to_owned(),
            value: 1,
            detail: "keeper_only_detail".to_owned(),
        },
        ActorRole::System,
        AuthorityMode::HumanKp,
    )
}

#[test]
fn observability_impl_rejects_authority_contract_violation() {
    let command = CommandEnvelope::governed(
        command().payload,
        ActorRole::AiKeeper,
        AuthorityMode::HumanKp,
    );
    let mut repository = ObservabilityRepository::default();

    let err = record_platform_observation(&mut repository, &command)
        .expect_err("authority mismatch denied");

    assert_eq!(err, TrpgError::AuthorityViolation);
    assert!(repository.events().is_empty());
}

#[test]
fn observability_impl_keeps_visibility_and_fact_provenance_on_replay() {
    let mut command = command();
    command.visibility = Visibility::new(VisibilityLabel::KeeperOnly);
    let mut repository = ObservabilityRepository::default();

    let event =
        record_platform_observation(&mut repository, &command).expect("observation recorded");

    assert_eq!(event.event_type, PLATFORM_OBSERVATION_RECORDED_EVENT);
    assert_eq!(event.fact_provenance, command.fact_provenance);
    assert!(repository
        .replay_visible(&PrincipalScope::Public)
        .is_empty());
    assert_eq!(repository.replay_visible(&PrincipalScope::System).len(), 1);
    assert!(matches!(
        event.payload,
        ObservabilityEvent::PlatformObservationRecorded { detail, .. } if detail == "[redacted]"
    ));
}

#[test]
fn observability_impl_requires_current_platform_metric_prefix() {
    let mut command = command();
    command.payload.metric_name = "command_total".to_owned();
    let mut repository = ObservabilityRepository::default();

    let err = record_platform_observation(&mut repository, &command)
        .expect_err("non-platform metric denied");

    assert_eq!(
        err,
        TrpgError::InvalidConfiguration("platform_metric_name_required")
    );
    assert_eq!(OBSERVABILITY_IMPL_METRIC_MODULE, "observability_impl");
}
