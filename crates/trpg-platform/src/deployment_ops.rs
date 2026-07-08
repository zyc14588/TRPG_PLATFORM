use crate::readme::{
    append_platform_event, PlatformEvent, PlatformEventEnvelope, PlatformEventStore,
};
use trpg_shared_kernel::{CommandEnvelope, KernelResult, TrpgError};

pub const DEPLOYMENT_CONFIGURED_EVENT: &str = "platform.deployment.configured";

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeploymentEnvironment {
    Development,
    Production,
}

impl DeploymentEnvironment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Development => "development",
            Self::Production => "production",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderEndpoint {
    pub provider: String,
    pub api_key: String,
    pub base_url: String,
    pub authenticated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigureDeployment {
    pub environment: DeploymentEnvironment,
    pub endpoint: ProviderEndpoint,
}

pub fn validate_provider_boundary(
    environment: &DeploymentEnvironment,
    endpoint: &ProviderEndpoint,
) -> KernelResult<()> {
    if environment == &DeploymentEnvironment::Production && placeholder_key(&endpoint.api_key) {
        return Err(TrpgError::InvalidConfiguration(
            "placeholder_api_key_forbidden_in_production",
        ));
    }

    let local_provider = local_provider(&endpoint.provider);
    let public_local_url = !loopback_base_url(&endpoint.base_url);
    if environment == &DeploymentEnvironment::Production
        && local_provider
        && (!endpoint.authenticated || public_local_url)
    {
        return Err(TrpgError::PolicyDenied);
    }

    Ok(())
}

pub fn configure_deployment(
    store: &mut PlatformEventStore,
    command: &CommandEnvelope<ConfigureDeployment>,
) -> KernelResult<PlatformEventEnvelope> {
    validate_provider_boundary(&command.payload.environment, &command.payload.endpoint)?;

    append_platform_event(
        store,
        command,
        DEPLOYMENT_CONFIGURED_EVENT,
        PlatformEvent::DeploymentConfigured {
            environment: command.payload.environment.as_str().to_owned(),
            provider: command.payload.endpoint.provider.clone(),
        },
    )
}

fn placeholder_key(api_key: &str) -> bool {
    let normalized = api_key.trim().to_ascii_lowercase();
    normalized.is_empty()
        || matches!(
            normalized.as_str(),
            "placeholder"
                | "changeme"
                | "change_me"
                | "test"
                | "example"
                | "ollama"
                | "sk-no-key-required"
        )
}

fn local_provider(provider: &str) -> bool {
    let normalized = provider.trim().to_ascii_lowercase();
    normalized.contains("local")
        || normalized.contains("ollama")
        || normalized.contains("llama_cpp")
        || normalized.contains("llama.cpp")
}

fn loopback_base_url(base_url: &str) -> bool {
    let normalized = base_url.trim().to_ascii_lowercase();
    let without_scheme = normalized
        .split_once("://")
        .map(|(_, rest)| rest)
        .unwrap_or(normalized.as_str());
    let authority = without_scheme.split('/').next().unwrap_or_default();
    let host_port = authority.rsplit('@').next().unwrap_or(authority);
    let host = host_port
        .split(':')
        .next()
        .unwrap_or_default()
        .trim_matches(['[', ']']);

    matches!(host, "localhost" | "127.0.0.1")
}
