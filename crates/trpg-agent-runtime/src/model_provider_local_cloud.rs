use crate::agent_runtime::AgentResult;
use crate::model_provider::{
    evaluate_cloud_fallback, CloudContextFact, CloudEgressAuthorization, FallbackDecision,
    ModelRouteSnapshot, ProviderConfig,
};

pub fn enforce_no_silent_cloud_fallback(
    source: &ProviderConfig,
    target: &ProviderConfig,
    route: &ModelRouteSnapshot,
    authorization: Option<CloudEgressAuthorization>,
    context: &[CloudContextFact],
) -> AgentResult<FallbackDecision> {
    evaluate_cloud_fallback(source, target, route, authorization, context)
}
