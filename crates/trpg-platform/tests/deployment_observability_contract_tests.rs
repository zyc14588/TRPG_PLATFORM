use trpg_platform::deployment_observability::{
    observe_deployment_health, ObserveDeploymentHealth, DEPLOYMENT_HEALTH_OBSERVED_EVENT,
};
use trpg_platform::{PlatformEvent, PlatformEventStore};
use trpg_shared_kernel::{ActorRole, AuthorityMode, TrpgError, Visibility, VisibilityLabel};

#[test]
fn service_without_healthcheck_is_rejected() {
    let command = trpg_test_support::governed_command(
        ObserveDeploymentHealth {
            service: "api".to_owned(),
            healthy: true,
            has_healthcheck: false,
            detail: "ok".to_owned(),
        },
        ActorRole::System,
        AuthorityMode::HumanKp,
    );
    let mut store = PlatformEventStore::default();

    let err = observe_deployment_health(&mut store, &command).expect_err("healthcheck required");

    assert_eq!(
        err,
        TrpgError::InvalidConfiguration("service_healthcheck_required")
    );
}

#[test]
fn deployment_health_detail_respects_visibility() {
    let mut command = trpg_test_support::governed_command(
        ObserveDeploymentHealth {
            service: "worker".to_owned(),
            healthy: true,
            has_healthcheck: true,
            detail: "private deployment note".to_owned(),
        },
        ActorRole::System,
        AuthorityMode::HumanKp,
    );
    command.visibility = Visibility::new(VisibilityLabel::SystemPrivate);
    let mut store = PlatformEventStore::default();

    let event =
        observe_deployment_health(&mut store, &command).expect("health observation recorded");

    assert_eq!(event.event_type, DEPLOYMENT_HEALTH_OBSERVED_EVENT);
    assert!(matches!(
        event.payload,
        PlatformEvent::DeploymentHealthObserved { detail, .. } if detail == "[redacted]"
    ));
}
