use trpg_platform::deployment_ops::{DeploymentEnvironment, ProviderEndpoint};
use trpg_platform::deployment_ops_impl::{
    apply_deployment_operation, ApplyDeploymentOperation, DeploymentOpsRepository,
    DEPLOYMENT_OPERATION_APPLIED_EVENT, DEPLOYMENT_OPS_IMPL_METRIC_MODULE,
};
use trpg_shared_kernel::{
    ActorRole, AuthorityMode, CommandEnvelope, PrincipalScope, TrpgError, Visibility,
    VisibilityLabel,
};

fn endpoint() -> ProviderEndpoint {
    ProviderEndpoint {
        provider: "cloud-provider".to_owned(),
        api_key: "real_key".to_owned(),
        base_url: "https://provider.example/v1".to_owned(),
        authenticated: true,
    }
}

fn command() -> CommandEnvelope<ApplyDeploymentOperation> {
    trpg_test_support::governed_command(
        ApplyDeploymentOperation {
            deployment_id: "deployment_001".to_owned(),
            environment: DeploymentEnvironment::Production,
            endpoint: endpoint(),
        },
        ActorRole::System,
        AuthorityMode::HumanKp,
    )
}

#[test]
fn deployment_ops_impl_rejects_authority_contract_violation() {
    let command = trpg_test_support::governed_command(
        command().payload,
        ActorRole::AiKeeper,
        AuthorityMode::HumanKp,
    );
    let mut repository = DeploymentOpsRepository::default();

    let err = apply_deployment_operation(&mut repository, &command)
        .expect_err("authority mismatch denied");

    assert_eq!(err, TrpgError::AuthorityViolation);
    assert!(repository.events().is_empty());
}

#[test]
fn deployment_ops_impl_keeps_visibility_and_fact_provenance_on_replay() {
    let mut command = command();
    command.visibility = Visibility::new(VisibilityLabel::SystemPrivate);
    let mut repository = DeploymentOpsRepository::default();

    let event = apply_deployment_operation(&mut repository, &command).expect("deployment evented");

    assert_eq!(event.event_type, DEPLOYMENT_OPERATION_APPLIED_EVENT);
    assert_eq!(event.fact_provenance, command.fact_provenance);
    assert!(repository
        .replay_visible(&PrincipalScope::Public)
        .is_empty());
    assert_eq!(repository.replay_visible(&PrincipalScope::System).len(), 1);
}

#[test]
fn deployment_ops_impl_rejects_public_unauthenticated_local_provider() {
    let mut command = command();
    command.payload.endpoint = ProviderEndpoint {
        provider: "local-openai-compatible".to_owned(),
        api_key: "real_key".to_owned(),
        base_url: "http://0.0.0.0:11434/v1".to_owned(),
        authenticated: false,
    };
    let mut repository = DeploymentOpsRepository::default();

    let err = apply_deployment_operation(&mut repository, &command)
        .expect_err("public local provider denied");

    assert_eq!(err, TrpgError::PolicyDenied);
    assert!(repository.events().is_empty());
    assert_eq!(DEPLOYMENT_OPS_IMPL_METRIC_MODULE, "deployment_ops_impl");
}
