use crate::agent_runtime::AgentResult;
use crate::model_provider::{
    evaluate_cloud_fallback, CloudContextFact, CloudEgressAuthorization, ExecutableModelProvider,
    ExecutedModelRouteSnapshot, FallbackDecision, ModelChatRequest, ModelChatResponse,
    ModelEmbeddingRequest, ModelEmbeddingResponse, ModelProviderError, ModelProviderErrorKind,
    ModelProviderResult, ModelRouteSnapshot, ModelStreamSink, ProviderCancellation, ProviderConfig,
    ProviderExecution, ProviderType,
};
use std::sync::Arc;

pub fn enforce_no_silent_cloud_fallback(
    source: &ProviderConfig,
    target: &ProviderConfig,
    route: &ModelRouteSnapshot,
    authorization: Option<CloudEgressAuthorization>,
    context: &[CloudContextFact],
) -> AgentResult<FallbackDecision> {
    evaluate_cloud_fallback(source, target, route, authorization, context)
}

/// Routes only to the provider named by a persisted route decision. Errors are
/// returned to the caller; this type deliberately has no fallback list.
pub struct ExplicitModelProviderRouter {
    providers: Vec<Arc<dyn ExecutableModelProvider>>,
}

impl ExplicitModelProviderRouter {
    pub fn new(providers: Vec<Arc<dyn ExecutableModelProvider>>) -> ModelProviderResult<Self> {
        if providers.is_empty()
            || providers.iter().enumerate().any(|(index, provider)| {
                providers[..index]
                    .iter()
                    .any(|existing| existing.provider_id() == provider.provider_id())
            })
        {
            return Err(route_configuration_error());
        }
        Ok(Self { providers })
    }

    pub fn provider_count(&self) -> usize {
        self.providers.len()
    }

    pub fn configured_provider_types(&self) -> Vec<ProviderType> {
        self.providers
            .iter()
            .map(|provider| provider.provider_type())
            .collect()
    }

    pub fn startup_route_snapshots(&self) -> Vec<ExecutedModelRouteSnapshot> {
        self.providers
            .iter()
            .map(|provider| provider.startup_route_snapshot())
            .collect()
    }

    pub async fn chat(
        &self,
        provider_id: &trpg_shared_kernel::EntityId,
        request: &ModelChatRequest,
        cancellation: &ProviderCancellation,
    ) -> ModelProviderResult<ProviderExecution<ModelChatResponse>> {
        self.provider(provider_id)?
            .chat(request, cancellation)
            .await
    }

    pub async fn stream_chat(
        &self,
        provider_id: &trpg_shared_kernel::EntityId,
        request: &ModelChatRequest,
        sink: &dyn ModelStreamSink,
        cancellation: &ProviderCancellation,
    ) -> ModelProviderResult<ExecutedModelRouteSnapshot> {
        self.provider(provider_id)?
            .stream_chat(request, sink, cancellation)
            .await
    }

    pub async fn embed(
        &self,
        provider_id: &trpg_shared_kernel::EntityId,
        request: &ModelEmbeddingRequest,
        cancellation: &ProviderCancellation,
    ) -> ModelProviderResult<ProviderExecution<ModelEmbeddingResponse>> {
        self.provider(provider_id)?
            .embed(request, cancellation)
            .await
    }

    fn provider(
        &self,
        provider_id: &trpg_shared_kernel::EntityId,
    ) -> ModelProviderResult<&dyn ExecutableModelProvider> {
        self.providers
            .iter()
            .find(|provider| provider.provider_id() == provider_id)
            .map(AsRef::as_ref)
            .ok_or_else(|| {
                ModelProviderError::new(
                    ModelProviderErrorKind::Configuration,
                    "MODEL_PROVIDER_ROUTE_NOT_CONFIGURED",
                    false,
                    None,
                )
            })
    }
}

fn route_configuration_error() -> ModelProviderError {
    ModelProviderError::new(
        ModelProviderErrorKind::Configuration,
        "MODEL_PROVIDER_ROUTER_CONFIGURATION_INVALID",
        false,
        None,
    )
}
