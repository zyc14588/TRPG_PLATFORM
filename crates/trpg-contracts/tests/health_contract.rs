use trpg_contracts::{ComponentCheck, HealthState, RoleRuntimeProbe, ServiceKind, ServicePhase};

#[test]
fn readiness_is_derived_from_phase_and_component_checks() {
    let ready = HealthState::new(
        ServiceKind::ApiServer,
        "test",
        ServicePhase::Ready,
        vec![ComponentCheck::passing("contract", "validated")],
    );
    assert!(ready.ready());
    assert_eq!(ready.ready_document()["status"], "ready");

    let degraded = HealthState::new(
        ServiceKind::ApiServer,
        "test",
        ServicePhase::Degraded,
        vec![ComponentCheck::failing("contract", "invalid")],
    );
    assert!(!degraded.ready());
    assert_eq!(degraded.ready_document()["status"], "not_ready");
    assert_eq!(degraded.ready_document()["checks"][0]["status"], "fail");
}

#[test]
fn liveness_requires_an_active_listener_and_non_stopping_phase() {
    let health = HealthState::new(
        ServiceKind::AgentWorker,
        "test",
        ServicePhase::Ready,
        Vec::new(),
    );
    assert!(health.live(true));
    assert!(!health.live(false));
}

#[test]
fn runtime_readiness_fails_after_the_role_loop_stops() {
    let mut probe =
        RoleRuntimeProbe::spawn("test_runtime", || Ok("responsive".to_owned())).unwrap();
    assert!(probe.component_check().ready);

    probe.shutdown();

    let stopped = probe.component_check();
    assert!(!stopped.ready);
    assert_eq!(stopped.detail, "runtime loop is not running");
}
