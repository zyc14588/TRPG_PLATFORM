use trpg_platform::policy_authz::{
    evaluate_platform_authorization, EvaluatePlatformAuthorization, PolicyAuthzRepository,
    PolicyGateDecision, PLATFORM_AUTHORIZATION_GRANTED_EVENT, POLICY_AUTHZ_METRIC_MODULE,
    POLICY_AUTHZ_REQUIRED_METRICS,
};
use trpg_shared_kernel::{
    ActorRole, AuthorityMode, CommandEnvelope, PrincipalScope, TrpgError, Visibility,
    VisibilityLabel,
};

fn command() -> CommandEnvelope<EvaluatePlatformAuthorization> {
    trpg_test_support::governed_command(
        EvaluatePlatformAuthorization {
            principal: "system".to_owned(),
            resource: "platform_events".to_owned(),
            action: "append".to_owned(),
            openfga_decision: PolicyGateDecision::Permit,
            opa_decision: PolicyGateDecision::Permit,
        },
        ActorRole::System,
        AuthorityMode::HumanKp,
    )
}

#[test]
fn policy_authz_rejects_authority_contract_violation() {
    let command = trpg_test_support::governed_command(
        command().payload,
        ActorRole::AiKeeper,
        AuthorityMode::HumanKp,
    );
    let mut repository = PolicyAuthzRepository::default();

    let err = evaluate_platform_authorization(&mut repository, &command)
        .expect_err("authority mismatch denied");

    assert_eq!(err, TrpgError::AuthorityViolation);
    assert!(repository.events().is_empty());
}

#[test]
fn policy_authz_keeps_visibility_and_fact_provenance_on_replay() {
    let mut command = command();
    command.visibility = Visibility::new(VisibilityLabel::KeeperOnly);
    let mut repository = PolicyAuthzRepository::default();

    let event = evaluate_platform_authorization(&mut repository, &command).expect("authz recorded");

    assert_eq!(event.event_type, PLATFORM_AUTHORIZATION_GRANTED_EVENT);
    assert_eq!(event.fact_provenance, command.fact_provenance);
    assert!(repository
        .replay_visible(&PrincipalScope::Public)
        .is_empty());
    assert_eq!(repository.replay_visible(&PrincipalScope::System).len(), 1);
}

#[test]
fn policy_authz_fails_closed_on_openfga_or_opa_deny() {
    for (openfga_decision, opa_decision) in [
        (PolicyGateDecision::Deny, PolicyGateDecision::Permit),
        (PolicyGateDecision::Permit, PolicyGateDecision::Deny),
    ] {
        let mut command = command();
        command.payload.openfga_decision = openfga_decision;
        command.payload.opa_decision = opa_decision;
        let mut repository = PolicyAuthzRepository::default();

        let err = evaluate_platform_authorization(&mut repository, &command)
            .expect_err("deny decision enforced");

        assert_eq!(err, TrpgError::PolicyDenied);
        assert!(repository.events().is_empty());
    }
}

#[test]
fn policy_authz_uses_current_safe_event_and_metric_names() {
    assert_eq!(POLICY_AUTHZ_METRIC_MODULE, "policy_authz");
    assert_eq!(
        PLATFORM_AUTHORIZATION_GRANTED_EVENT,
        "platform.policy_authz.authorization_granted"
    );
    assert!(!PLATFORM_AUTHORIZATION_GRANTED_EVENT.contains("impl"));
    assert!(POLICY_AUTHZ_REQUIRED_METRICS.contains(&"trpg_command_total"));
    assert!(POLICY_AUTHZ_REQUIRED_METRICS.contains(&"trpg_visibility_redaction_total"));
}
