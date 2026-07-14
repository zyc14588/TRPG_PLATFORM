use crate::agent_runtime::AgentResult;
use crate::model_provider::{
    evaluate_cloud_fallback, FallbackDecision, FallbackPolicy, ProviderType,
};

pub fn enforce_no_silent_cloud_fallback(
    from: ProviderType,
    to: ProviderType,
    policy: FallbackPolicy,
) -> AgentResult<FallbackDecision> {
    evaluate_cloud_fallback(from, to, policy)
}
