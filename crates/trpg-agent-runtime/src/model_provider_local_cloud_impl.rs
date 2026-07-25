use crate::agent_runtime::AgentResult;
use crate::local_model_certification::{
    ensure_ai_keeper_model, LocalModelCertificate, LocalModelCertificationAuthority,
};
use crate::model_provider::{
    evaluate_cloud_fallback, provider_boundary_snapshot, validate_provider_config,
    CloudContextFact, CloudEgressAuthorization, FallbackDecision, ModelProviderBoundarySnapshot,
    ModelRouteSnapshot, ProviderConfig,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderRouteEvaluation {
    pub boundary: ModelProviderBoundarySnapshot,
    pub fallback: FallbackDecision,
    pub ai_keeper_allowed: bool,
}

pub fn evaluate_provider_route_for_ai_keeper(
    source: &ProviderConfig,
    target: &ProviderConfig,
    route: &ModelRouteSnapshot,
    authorization: Option<CloudEgressAuthorization>,
    context: &[CloudContextFact],
    certification_authority: &LocalModelCertificationAuthority,
    local_model_certificate: &LocalModelCertificate,
) -> AgentResult<ProviderRouteEvaluation> {
    validate_provider_config(source)?;
    validate_provider_config(target)?;
    let fallback = evaluate_cloud_fallback(source, target, route, authorization, context)?;
    ensure_ai_keeper_model(
        certification_authority,
        local_model_certificate,
        &source.model_id,
        &source.model_artifact_sha256,
    )?;

    Ok(ProviderRouteEvaluation {
        boundary: provider_boundary_snapshot(),
        fallback,
        ai_keeper_allowed: true,
    })
}
