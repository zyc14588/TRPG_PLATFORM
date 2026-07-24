use trpg_agent_runtime::model_provider::{
    validate_provider_config, Environment, ProviderConfig, ProviderType, SecretReference,
};

#[test]
fn provider_configuration_debug_contains_only_a_redacted_reference() {
    let config = ProviderConfig {
        provider_id: trpg_shared_kernel::EntityId::new("cloud-provider").unwrap(),
        provider_type: ProviderType::Cloud,
        model_id: "provider-model".to_owned(),
        model_artifact_sha256: format!("sha256:{}", "a".repeat(64)),
        base_url: "https://provider.example/v1".to_owned(),
        credential: SecretReference::kms("customer_provider_credential", 7).unwrap(),
        environment: Environment::Prod,
    };

    validate_provider_config(&config).unwrap();
    let debug = format!("{config:?}");
    assert!(!debug.contains("customer_provider_credential"));
    assert!(debug.contains("[redacted]"));
    assert!(!debug.contains("api_key"));
    assert!(!debug.contains("provider.example"));
}

#[test]
fn provider_endpoint_rejects_secret_carriers_and_plaintext_production_transport() {
    for base_url in [
        "https://token@provider.example/v1",
        "https://provider.example/v1?api_key=secret",
        "http://provider.example/v1",
    ] {
        let config = ProviderConfig {
            provider_id: trpg_shared_kernel::EntityId::new("cloud-provider").unwrap(),
            provider_type: ProviderType::Cloud,
            model_id: "provider-model".to_owned(),
            model_artifact_sha256: format!("sha256:{}", "a".repeat(64)),
            base_url: base_url.to_owned(),
            credential: SecretReference::kms("cloud_provider", 1).unwrap(),
            environment: Environment::Prod,
        };
        let debug = format!("{config:?}");
        assert!(!debug.contains("token"));
        assert!(!debug.contains("api_key"));
        assert!(validate_provider_config(&config).is_err());
    }
}

#[test]
fn production_provider_rejects_development_memory_credentials() {
    let config = ProviderConfig {
        provider_id: trpg_shared_kernel::EntityId::new("cloud-provider").unwrap(),
        provider_type: ProviderType::Cloud,
        model_id: "provider-model".to_owned(),
        model_artifact_sha256: format!("sha256:{}", "a".repeat(64)),
        base_url: "https://provider.example/v1".to_owned(),
        credential: SecretReference::development("dev_provider", 1).unwrap(),
        environment: Environment::Prod,
    };

    assert_eq!(
        validate_provider_config(&config).unwrap_err().code(),
        "INVALID_CONFIGURATION"
    );
}

#[test]
fn a_local_provider_label_cannot_hide_a_remote_https_endpoint() {
    let config = ProviderConfig {
        provider_id: trpg_shared_kernel::EntityId::new("forged-local-label").unwrap(),
        provider_type: ProviderType::Ollama,
        model_id: "provider-model".to_owned(),
        model_artifact_sha256: format!("sha256:{}", "a".repeat(64)),
        base_url: "https://remote-provider.example/v1".to_owned(),
        credential: SecretReference::development("forged_local", 1).unwrap(),
        environment: Environment::Dev,
    };

    assert_eq!(
        validate_provider_config(&config).unwrap_err().code(),
        "UNAUTHENTICATED_LOCAL_PROVIDER_EXPOSED"
    );
}
