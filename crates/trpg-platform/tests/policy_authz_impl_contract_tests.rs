use trpg_platform::policy_authz_impl::{
    evaluate_platform_authorization, AuthorizationDecision, EvaluatePlatformAuthorization,
    PolicyAuthzRepository, PLATFORM_AUTHORIZATION_GRANTED_EVENT, POLICY_AUTHZ_IMPL_METRIC_MODULE,
};
use trpg_shared_kernel::{
    ActorRole, AuthorityMode, CommandEnvelope, PrincipalScope, TrpgError, Visibility,
    VisibilityLabel,
};

fn command() -> CommandEnvelope<EvaluatePlatformAuthorization> {
    trpg_test_support::governed_command!(
        EvaluatePlatformAuthorization {
            principal: "system".to_owned(),
            resource: "platform_events".to_owned(),
            action: "append".to_owned(),
            decision: AuthorizationDecision::Permit,
        },
        ActorRole::System,
        AuthorityMode::HumanKp,
    )
}

#[test]
fn policy_authz_impl_rejects_authority_contract_violation() {
    let command = trpg_test_support::governed_command!(
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
fn policy_authz_impl_keeps_visibility_and_fact_provenance_on_replay() {
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
fn policy_authz_impl_rejects_policy_deny_decision() {
    let mut command = command();
    command.payload.decision = AuthorizationDecision::Deny;
    let mut repository = PolicyAuthzRepository::default();

    let err = evaluate_platform_authorization(&mut repository, &command)
        .expect_err("deny decision enforced");

    assert_eq!(err, TrpgError::PolicyDenied);
    assert!(repository.events().is_empty());
    assert_eq!(POLICY_AUTHZ_IMPL_METRIC_MODULE, "policy_authz_impl");
}
