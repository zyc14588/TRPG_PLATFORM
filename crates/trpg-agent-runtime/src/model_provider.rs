use crate::agent_runtime::{AgentError, AgentResult};

pub const PROMPT_ID: &str = "CODEX-0046-04-AI-AGENT-SYSTEM-6468c9be5b";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProviderType {
    Cloud,
    Ollama,
    LlamaCpp,
    LocalOpenAiCompatible,
}

impl ProviderType {
    pub fn is_local(self) -> bool {
        !matches!(self, Self::Cloud)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Environment {
    Dev,
    Prod,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderConfig {
    pub provider_type: ProviderType,
    pub base_url: String,
    pub api_key: String,
    pub environment: Environment,
    pub reverse_proxy_auth: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelRouteSnapshot {
    pub provider_type: ProviderType,
    pub model_id: String,
    pub fallback_policy: &'static str,
    pub privacy_boundary: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FallbackDecision {
    Allow,
    DenyAndAudit,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FallbackPolicy {
    pub cloud_fallback_enabled: bool,
    pub user_notice: bool,
    pub snapshot_recorded: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModelProviderBoundarySnapshot {
    pub gateway: &'static str,
    pub runtime: &'static str,
    pub provider_adapter: &'static str,
    pub forbidden_direct_call_error: &'static str,
}

pub fn provider_boundary_snapshot() -> ModelProviderBoundarySnapshot {
    ModelProviderBoundarySnapshot {
        gateway: "Agent Gateway",
        runtime: "Agent Orchestrator/Runtime",
        provider_adapter: "Model Provider Adapter",
        forbidden_direct_call_error: AgentError::DirectLlmCallForbidden.code(),
    }
}

pub fn validate_provider_config(config: &ProviderConfig) -> AgentResult<()> {
    if config.environment == Environment::Prod
        && config.provider_type.is_local()
        && (config.base_url.contains("0.0.0.0") || !config.reverse_proxy_auth)
    {
        return Err(AgentError::UnauthenticatedLocalProviderExposed);
    }

    Ok(())
}

pub fn evaluate_cloud_fallback(
    from: ProviderType,
    to: ProviderType,
    policy: FallbackPolicy,
) -> AgentResult<FallbackDecision> {
    if from.is_local()
        && to == ProviderType::Cloud
        && !(policy.cloud_fallback_enabled && policy.user_notice && policy.snapshot_recorded)
    {
        return Err(AgentError::SilentFallbackForbidden);
    }

    Ok(if from.is_local() && to == ProviderType::Cloud {
        FallbackDecision::Allow
    } else {
        FallbackDecision::DenyAndAudit
    })
}
