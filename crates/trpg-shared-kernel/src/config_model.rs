use crate::shared_kernel::{KernelResult, TrpgError};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelAccessPath {
    AgentGateway,
    DirectCloudProvider,
    DirectLocalProvider,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeEnvironment {
    Development,
    Production,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LocalModelCertification {
    Level0,
    Level1,
    Level2,
    Level3,
    Level4,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuntimeConfig {
    pub environment: RuntimeEnvironment,
    pub model_access_path: ModelAccessPath,
    pub placeholder_api_key: bool,
    pub local_model_certification: LocalModelCertification,
    pub local_model_as_ai_keeper: bool,
    pub cross_privacy_fallback_explicit: bool,
}

pub fn validate_runtime_config(config: &RuntimeConfig) -> KernelResult<()> {
    if config.model_access_path != ModelAccessPath::AgentGateway {
        return Err(TrpgError::InvalidConfiguration(
            "model providers must be reached through Agent Gateway",
        ));
    }

    if config.environment == RuntimeEnvironment::Production && config.placeholder_api_key {
        return Err(TrpgError::InvalidConfiguration(
            "production must not accept placeholder API keys",
        ));
    }

    if config.local_model_as_ai_keeper
        && config.local_model_certification < LocalModelCertification::Level4
    {
        return Err(TrpgError::InvalidConfiguration(
            "AI Keeper local model requires Level 4 certification",
        ));
    }

    if !config.cross_privacy_fallback_explicit {
        return Err(TrpgError::InvalidConfiguration(
            "cross privacy boundary fallback must be explicit",
        ));
    }

    Ok(())
}
