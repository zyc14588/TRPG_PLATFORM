use trpg_platform::deployment_ops::{
    configure_deployment, validate_provider_boundary, ConfigureDeployment, DeploymentEnvironment,
    KmsClient, KmsSecretResolver, ProviderEndpoint, SecretManager, SecretReference,
    DEPLOYMENT_CONFIGURED_EVENT,
};
use trpg_platform::PlatformEventStore;
use trpg_shared_kernel::{ActorRole, AuthorityMode, KernelResult, TrpgError};

const MODEL_ARTIFACT_SHA256: &str =
    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

struct TestKms;

impl KmsClient for TestKms {
    fn decrypt_secret(&self, _secret_id: &str, _version: u64) -> KernelResult<Vec<u8>> {
        Ok(b"resolved-provider-credential".to_vec())
    }
}

fn manager_for(reference: &SecretReference) -> SecretManager<KmsSecretResolver<TestKms>> {
    let manager = SecretManager::new(KmsSecretResolver::new(TestKms));
    manager.register(reference).unwrap();
    manager
}

fn endpoint(provider: &str, credential: SecretReference) -> ProviderEndpoint {
    endpoint_with_base_url(provider, credential, "https://provider.example/v1")
}

fn endpoint_with_base_url(
    provider: &str,
    credential: SecretReference,
    base_url: &str,
) -> ProviderEndpoint {
    ProviderEndpoint::new(
        provider,
        base_url,
        credential,
        DeploymentEnvironment::Production,
        "provider-model-v1",
        MODEL_ARTIFACT_SHA256,
    )
    .unwrap()
}

#[test]
fn production_rejects_development_secret_backend() {
    let manager = manager_for(&SecretReference::kms("registered_control", 1).unwrap());
    let err = validate_provider_boundary(
        &DeploymentEnvironment::Production,
        &endpoint(
            "model-provider",
            SecretReference::development("provider_dev", 1).unwrap(),
        ),
        &manager,
    )
    .expect_err("placeholder key denied");

    assert_eq!(
        err,
        TrpgError::InvalidConfiguration("production_secret_backend_required")
    );
}

#[test]
fn production_rejects_all_development_only_secret_references() {
    let manager = manager_for(&SecretReference::kms("registered_control", 1).unwrap());
    for secret_id in ["ollama_dev", "cloud_dev"] {
        let err = validate_provider_boundary(
            &DeploymentEnvironment::Production,
            &endpoint(
                "cloud-provider",
                SecretReference::development(secret_id, 1).unwrap(),
            ),
            &manager,
        )
        .expect_err("dev placeholder key denied");

        assert_eq!(
            err,
            TrpgError::InvalidConfiguration("production_secret_backend_required")
        );
    }
}

#[test]
fn production_rejects_non_loopback_local_provider_even_with_a_secret_reference() {
    let manager = manager_for(&SecretReference::kms("registered_control", 1).unwrap());
    let err = validate_provider_boundary(
        &DeploymentEnvironment::Production,
        &endpoint(
            "local-model-provider",
            SecretReference::mounted("local_provider", 1).unwrap(),
        ),
        &manager,
    )
    .expect_err("non-loopback local provider denied");

    assert_eq!(
        err,
        TrpgError::InvalidConfiguration("unauthenticated_local_provider_exposed")
    );
}

#[test]
fn production_rejects_public_unauthenticated_local_llm_fixture_case() {
    let manager = manager_for(&SecretReference::kms("registered_control", 1).unwrap());
    let err = validate_provider_boundary(
        &DeploymentEnvironment::Production,
        &endpoint_with_base_url(
            "local-openai-compatible",
            SecretReference::mounted("local_provider", 1).unwrap(),
            "http://0.0.0.0:11434/v1",
        ),
        &manager,
    )
    .expect_err("public unauthenticated local LLM denied");

    assert_eq!(
        err,
        TrpgError::InvalidConfiguration("unauthenticated_local_provider_exposed")
    );
}

#[test]
fn deployment_configuration_is_evented() {
    let credential = SecretReference::kms("cloud_provider", 1).unwrap();
    let manager = manager_for(&credential);
    let command = trpg_test_support::governed_command(
        ConfigureDeployment {
            environment: DeploymentEnvironment::Production,
            endpoint: endpoint("cloud-provider", credential),
        },
        ActorRole::System,
        AuthorityMode::HumanKp,
    );
    let mut store = PlatformEventStore::default();

    let event =
        configure_deployment(&mut store, &command, &manager).expect("deployment configured");

    assert_eq!(event.event_type, DEPLOYMENT_CONFIGURED_EVENT);
    assert_eq!(store.events().len(), 1);
    let trpg_platform::PlatformEvent::DeploymentConfigured {
        security_snapshot_digest,
        ..
    } = &event.payload
    else {
        panic!("deployment event payload");
    };
    assert!(security_snapshot_digest.starts_with("sha256:"));
    assert_eq!(security_snapshot_digest.len(), 71);
}

#[test]
fn production_rejects_unknown_plaintext_or_credential_bearing_provider_endpoints() {
    let manager = manager_for(&SecretReference::kms("registered_control", 1).unwrap());
    for endpoint in [
        endpoint_with_base_url(
            "unknown-local-ish-provider",
            SecretReference::mounted("provider", 1).unwrap(),
            "https://provider.example/v1",
        ),
        endpoint_with_base_url(
            "cloud-provider",
            SecretReference::kms("provider", 1).unwrap(),
            "http://provider.example/v1",
        ),
        endpoint_with_base_url(
            "cloud-provider",
            SecretReference::kms("provider", 1).unwrap(),
            "https://token@provider.example/v1",
        ),
    ] {
        let debug = format!("{endpoint:?}");
        assert!(!debug.contains("token"));
        assert!(!debug.contains("provider.example"));
        assert!(validate_provider_boundary(
            &DeploymentEnvironment::Production,
            &endpoint,
            &manager
        )
        .is_err());
    }
}

#[test]
fn deployment_rejects_unresolved_or_environment_mismatched_credentials() {
    let registered = SecretReference::kms("registered_control", 1).unwrap();
    let manager = manager_for(&registered);
    let unresolved = SecretReference::kms("missing_provider", 1).unwrap();
    let endpoint = endpoint("cloud-provider", unresolved);

    assert_eq!(
        validate_provider_boundary(&DeploymentEnvironment::Production, &endpoint, &manager)
            .expect_err("a locator is not proof of an active resolvable credential"),
        TrpgError::AuthorizationDenied
    );

    assert_eq!(
        validate_provider_boundary(&DeploymentEnvironment::Development, &endpoint, &manager)
            .expect_err("command environment must equal the attested endpoint environment"),
        TrpgError::InvalidConfiguration("deployment_environment_binding_mismatch")
    );
}
