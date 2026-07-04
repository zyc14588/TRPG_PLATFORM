use crate::agent_runtime::AgentResult;
use crate::model_provider::{
    evaluate_cloud_fallback, FallbackDecision, FallbackPolicy, ProviderType,
};

pub const PROMPT_ID: &str = "CODEX-0454-04-AI-AGENT-SYSTEM-014bc53177";

pub fn enforce_no_silent_cloud_fallback(
    from: ProviderType,
    to: ProviderType,
    policy: FallbackPolicy,
) -> AgentResult<FallbackDecision> {
    evaluate_cloud_fallback(from, to, policy)
}
