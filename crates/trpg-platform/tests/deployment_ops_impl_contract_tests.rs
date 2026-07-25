use trpg_platform::deployment_ops::{
    DeploymentEnvironment, KmsClient, KmsSecretResolver, ProviderEndpoint, SecretManager,
    SecretReference,
};
use trpg_platform::deployment_ops_impl::{
    apply_deployment_operation, ApplyDeploymentOperation, DeploymentOpsRepository,
    DEPLOYMENT_OPERATION_APPLIED_EVENT, DEPLOYMENT_OPS_IMPL_METRIC_MODULE,
};
use trpg_shared_kernel::{
    ActorRole, AuthorityMode, CommandEnvelope, KernelResult, PrincipalScope, TrpgError, Visibility,
    VisibilityLabel,
};

const MODEL_ARTIFACT_SHA256: &str =
    "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

struct TestKms;

impl KmsClient for TestKms {
    fn decrypt_secret(&self, _secret_id: &str, _version: u64) -> KernelResult<Vec<u8>> {
        Ok(b"resolved-provider-credential".to_vec())
    }
}

fn manager() -> SecretManager<KmsSecretResolver<TestKms>> {
    let manager = SecretManager::new(KmsSecretResolver::new(TestKms));
    manager
        .register(&SecretReference::kms("cloud_provider", 1).unwrap())
        .unwrap();
    manager
}

fn endpoint() -> ProviderEndpoint {
    ProviderEndpoint::new(
        "cloud-provider",
        "https://provider.example/v1",
        SecretReference::kms("cloud_provider", 1).unwrap(),
        DeploymentEnvironment::Production,
        "provider-model-v1",
        MODEL_ARTIFACT_SHA256,
    )
    .unwrap()
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

    let err = apply_deployment_operation(&mut repository, &command, &manager())
        .expect_err("authority mismatch denied");

    assert_eq!(err, TrpgError::AuthorityViolation);
    assert!(repository.events().is_empty());
}

#[test]
fn deployment_ops_impl_keeps_visibility_and_fact_provenance_on_replay() {
    let mut command = command();
    command.visibility = Visibility::new(VisibilityLabel::SystemPrivate);
    let mut repository = DeploymentOpsRepository::default();

    let event = apply_deployment_operation(&mut repository, &command, &manager())
        .expect("deployment evented");

    assert_eq!(event.event_type, DEPLOYMENT_OPERATION_APPLIED_EVENT);
    assert_eq!(event.fact_provenance, command.fact_provenance);
    let trpg_platform::deployment_ops_impl::DeploymentOpsEvent::DeploymentOperationApplied {
        security_snapshot_digest,
        ..
    } = &event.payload;
    assert!(security_snapshot_digest.starts_with("sha256:"));
    assert!(repository
        .replay_visible(&PrincipalScope::Public)
        .is_empty());
    assert_eq!(repository.replay_visible(&PrincipalScope::System).len(), 1);
}

#[test]
fn deployment_ops_impl_rejects_public_unauthenticated_local_provider() {
    let mut command = command();
    command.payload.endpoint = ProviderEndpoint::new(
        "local-openai-compatible",
        "http://0.0.0.0:11434/v1",
        SecretReference::mounted("local_provider", 1).unwrap(),
        DeploymentEnvironment::Production,
        "local-model-v1",
        MODEL_ARTIFACT_SHA256,
    )
    .unwrap();
    let mut repository = DeploymentOpsRepository::default();

    let err = apply_deployment_operation(&mut repository, &command, &manager())
        .expect_err("public local provider denied");

    assert_eq!(
        err,
        TrpgError::InvalidConfiguration("unauthenticated_local_provider_exposed")
    );
    assert!(repository.events().is_empty());
    assert_eq!(DEPLOYMENT_OPS_IMPL_METRIC_MODULE, "deployment_ops_impl");
}
