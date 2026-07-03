use trpg_shared_kernel::config_model::{
    validate_runtime_config, LocalModelCertification, ModelAccessPath, RuntimeConfig,
    RuntimeEnvironment,
};
use trpg_shared_kernel::TrpgError;

fn valid_config() -> RuntimeConfig {
    RuntimeConfig {
        environment: RuntimeEnvironment::Production,
        model_access_path: ModelAccessPath::AgentGateway,
        placeholder_api_key: false,
        local_model_certification: LocalModelCertification::Level4,
        local_model_as_ai_keeper: true,
        cross_privacy_fallback_explicit: true,
    }
}

#[test]
fn config_model_requires_agent_gateway_access_path() {
    let mut config = valid_config();
    config.model_access_path = ModelAccessPath::DirectCloudProvider;

    assert!(matches!(
        validate_runtime_config(&config),
        Err(TrpgError::InvalidConfiguration(_))
    ));
}

#[test]
fn config_model_rejects_placeholder_keys_in_production() {
    let mut config = valid_config();
    config.placeholder_api_key = true;

    assert!(matches!(
        validate_runtime_config(&config),
        Err(TrpgError::InvalidConfiguration(_))
    ));
}

#[test]
fn config_model_requires_level_four_for_local_ai_keeper() {
    let mut config = valid_config();
    config.local_model_certification = LocalModelCertification::Level3;

    assert!(matches!(
        validate_runtime_config(&config),
        Err(TrpgError::InvalidConfiguration(_))
    ));
}
