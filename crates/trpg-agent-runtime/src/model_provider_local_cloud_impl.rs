use crate::agent_runtime::AgentResult;
use crate::local_model_certification::{ensure_ai_keeper_model, LocalModelLevel};
use crate::model_provider::{
    evaluate_cloud_fallback, provider_boundary_snapshot, validate_provider_config,
    FallbackDecision, FallbackPolicy, ModelProviderBoundarySnapshot, ProviderConfig, ProviderType,
};

pub const PROMPT_ID: &str = "CODEX-0484-04-AI-AGENT-SYSTEM-e96dc3868d";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderRouteEvaluation {
    pub boundary: ModelProviderBoundarySnapshot,
    pub fallback: FallbackDecision,
    pub ai_keeper_allowed: bool,
}

pub fn evaluate_provider_route_for_ai_keeper(
    config: &ProviderConfig,
    from: ProviderType,
    to: ProviderType,
    policy: FallbackPolicy,
    local_model_level: LocalModelLevel,
) -> AgentResult<ProviderRouteEvaluation> {
    validate_provider_config(config)?;
    let fallback = evaluate_cloud_fallback(from, to, policy)?;
    ensure_ai_keeper_model(local_model_level)?;

    Ok(ProviderRouteEvaluation {
        boundary: provider_boundary_snapshot(),
        fallback,
        ai_keeper_allowed: true,
    })
}
