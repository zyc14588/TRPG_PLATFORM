use crate::agent_runtime::AgentResult;
use crate::local_model_certification::{
    ensure_ai_keeper_model, LocalModelCertificate, LocalModelCertificationAuthority,
};
use crate::model_provider::{
    evaluate_cloud_fallback, provider_boundary_snapshot, validate_provider_config,
    CloudContextFact, CloudEgressAuthorization, ExecutableModelProvider,
    ExecutedModelRouteSnapshot, FallbackDecision, ModelChatRequest, ModelChatResponse,
    ModelEmbeddingRequest, ModelEmbeddingResponse, ModelOperation, ModelProviderError,
    ModelProviderErrorKind, ModelProviderResult, ModelProviderRuntimeConfig, ModelStreamChunk,
    ModelStreamSink, ModelTokenUsage, ModelToolCall, ProviderCancellation, ProviderCapabilities,
    ProviderConfig, ProviderExecution, ProviderType, RequiredProviderCapability,
};
use async_trait::async_trait;
use reqwest::header::{HeaderValue, AUTHORIZATION};
use reqwest::{Client, Method, RequestBuilder, Response, StatusCode};
use serde_json::{json, Map, Value};
use std::collections::HashSet;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use trpg_security_governance::secret::{SecretManager, SecretResolver};
use zeroize::Zeroizing;

const MAX_PROVIDER_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const MAX_MODEL_CONTEXT_BYTES: usize = 1024 * 1024;
const MAX_STREAM_LINE_BYTES: usize = 1024 * 1024;
const MAX_MESSAGES: usize = 128;
const MAX_TOOLS: usize = 64;
const MIN_REQUEST_TIMEOUT: Duration = Duration::from_millis(10);
const MAX_REQUEST_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProviderRouteEvaluation {
    pub boundary: crate::model_provider::ModelProviderBoundarySnapshot,
    pub fallback: FallbackDecision,
    pub ai_keeper_allowed: bool,
}

pub fn evaluate_provider_route_for_ai_keeper(
    source: &ProviderConfig,
    target: &ProviderConfig,
    route: &crate::model_provider::ModelRouteSnapshot,
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

include!("model_provider_local_cloud_impl_sections/01_http_client.rs");
include!("model_provider_local_cloud_impl_sections/02_request_stream.rs");
include!("model_provider_local_cloud_impl_sections/03_provider_trait.rs");
include!("model_provider_local_cloud_impl_sections/04_response_parsing.rs");
include!("model_provider_local_cloud_impl_sections/05_error_classification.rs");
