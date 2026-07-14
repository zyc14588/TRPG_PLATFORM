use trpg_platform::deployment_ops::{
    configure_deployment, validate_provider_boundary, ConfigureDeployment, DeploymentEnvironment,
    ProviderEndpoint, DEPLOYMENT_CONFIGURED_EVENT,
};
use trpg_platform::PlatformEventStore;
use trpg_shared_kernel::{ActorRole, AuthorityMode, TrpgError};

fn endpoint(provider: &str, api_key: &str, authenticated: bool) -> ProviderEndpoint {
    endpoint_with_base_url(provider, api_key, "http://127.0.0.1:11434", authenticated)
}

fn endpoint_with_base_url(
    provider: &str,
    api_key: &str,
    base_url: &str,
    authenticated: bool,
) -> ProviderEndpoint {
    ProviderEndpoint {
        provider: provider.to_owned(),
        api_key: api_key.to_owned(),
        base_url: base_url.to_owned(),
        authenticated,
    }
}

#[test]
fn production_rejects_placeholder_api_key() {
    let err = validate_provider_boundary(
        &DeploymentEnvironment::Production,
        &endpoint("model-provider", "placeholder", true),
    )
    .expect_err("placeholder key denied");

    assert_eq!(
        err,
        TrpgError::InvalidConfiguration("placeholder_api_key_forbidden_in_production")
    );
}

#[test]
fn production_rejects_dev_placeholder_api_keys() {
    for api_key in ["ollama", "sk-no-key-required"] {
        let err = validate_provider_boundary(
            &DeploymentEnvironment::Production,
            &endpoint("cloud-provider", api_key, true),
        )
        .expect_err("dev placeholder key denied");

        assert_eq!(
            err,
            TrpgError::InvalidConfiguration("placeholder_api_key_forbidden_in_production")
        );
    }
}

#[test]
fn production_rejects_unauthenticated_local_provider() {
    let err = validate_provider_boundary(
        &DeploymentEnvironment::Production,
        &endpoint("local-model-provider", "real_key", false),
    )
    .expect_err("unauthenticated local provider denied");

    assert_eq!(err, TrpgError::PolicyDenied);
}

#[test]
fn production_rejects_public_unauthenticated_local_llm_fixture_case() {
    let err = validate_provider_boundary(
        &DeploymentEnvironment::Production,
        &endpoint_with_base_url(
            "local-openai-compatible",
            "real_key",
            "http://0.0.0.0:11434/v1",
            false,
        ),
    )
    .expect_err("public unauthenticated local LLM denied");

    assert_eq!(err, TrpgError::PolicyDenied);
}

#[test]
fn deployment_configuration_is_evented() {
    let command = trpg_test_support::governed_command(
        ConfigureDeployment {
            environment: DeploymentEnvironment::Production,
            endpoint: endpoint("cloud-provider", "real_key", true),
        },
        ActorRole::System,
        AuthorityMode::HumanKp,
    );
    let mut store = PlatformEventStore::default();

    let event = configure_deployment(&mut store, &command).expect("deployment configured");

    assert_eq!(event.event_type, DEPLOYMENT_CONFIGURED_EVENT);
    assert_eq!(store.events().len(), 1);
}
