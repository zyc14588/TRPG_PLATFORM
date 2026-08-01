use crate::agent_runtime::{AgentError, AgentResult};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use trpg_security_governance::cloud_egress::CloudEgressAttempt;
pub use trpg_security_governance::cloud_egress::{CloudContextFact, CloudEgressAuthorization};
pub use trpg_security_governance::secret::SecretReference;

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

    pub const fn route_name(self) -> &'static str {
        match self {
            Self::Cloud => "cloud",
            Self::Ollama => "ollama",
            Self::LlamaCpp => "llama_cpp",
            Self::LocalOpenAiCompatible => "local_openai_compatible",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Environment {
    Dev,
    Prod,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ProviderConfig {
    pub provider_id: trpg_shared_kernel::EntityId,
    pub provider_type: ProviderType,
    pub model_id: String,
    pub model_artifact_sha256: String,
    pub base_url: String,
    pub credential: SecretReference,
    pub environment: Environment,
}

impl std::fmt::Debug for ProviderConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ProviderConfig")
            .field("provider_id", &self.provider_id)
            .field("provider_type", &self.provider_type)
            .field("model_id", &self.model_id)
            .field("model_artifact_sha256", &self.model_artifact_sha256)
            .field("base_url", &"[redacted endpoint]")
            .field("credential", &self.credential)
            .field("environment", &self.environment)
            .finish()
    }
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
    if config.model_id.trim().is_empty()
        || config.model_id.len() > 256
        || config.model_artifact_sha256.len() != 71
        || !config.model_artifact_sha256.starts_with("sha256:")
        || !config.model_artifact_sha256[7..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(AgentError::Core(
            trpg_shared_kernel::TrpgError::InvalidConfiguration("provider_model_identity_invalid"),
        ));
    }
    if config.environment == Environment::Prod && !config.credential.production_eligible() {
        return Err(AgentError::Core(
            trpg_shared_kernel::TrpgError::InvalidConfiguration(
                "production_secret_backend_required",
            ),
        ));
    }
    let endpoint = url::Url::parse(&config.base_url).map_err(|_| {
        AgentError::Core(trpg_shared_kernel::TrpgError::InvalidConfiguration(
            "provider_endpoint_invalid",
        ))
    })?;
    if endpoint.host_str().is_none()
        || !endpoint.username().is_empty()
        || endpoint.password().is_some()
        || endpoint.query().is_some()
        || endpoint.fragment().is_some()
    {
        return Err(AgentError::Core(
            trpg_shared_kernel::TrpgError::InvalidConfiguration(
                "provider_endpoint_must_not_contain_credentials",
            ),
        ));
    }
    let host_is_loopback = matches!(endpoint.host_str(), Some("localhost" | "127.0.0.1" | "::1"));
    if config.provider_type.is_local() && !host_is_loopback {
        return Err(AgentError::UnauthenticatedLocalProviderExposed);
    }
    if config.provider_type == ProviderType::Cloud && host_is_loopback {
        return Err(AgentError::Core(
            trpg_shared_kernel::TrpgError::InvalidConfiguration(
                "cloud_provider_endpoint_must_be_remote",
            ),
        ));
    }
    if config.environment == Environment::Prod && endpoint.scheme() != "https" {
        return Err(AgentError::Core(
            trpg_shared_kernel::TrpgError::InvalidConfiguration(
                "production_provider_https_required",
            ),
        ));
    }
    if config.provider_type.is_local() && config.environment == Environment::Prod {
        resolve_provider_runtime_sha256(config)?;
    }

    Ok(())
}

/// Resolves the identity bound to local-provider certification. Production
/// local providers must be pinned to the deployed runtime/container digest by
/// the process owner; development providers use a deterministic adapter/route
/// fingerprint so tests cannot substitute an applicant-supplied value.
pub fn resolve_provider_runtime_sha256(config: &ProviderConfig) -> AgentResult<String> {
    const RUNTIME_SHA256_ENV: &str = "TRPG_MODEL_PROVIDER_RUNTIME_SHA256";
    let runtime_pin = if config.provider_type.is_local() && config.environment == Environment::Prod
    {
        let value = std::env::var(RUNTIME_SHA256_ENV).map_err(|_| {
            AgentError::Core(trpg_shared_kernel::TrpgError::InvalidConfiguration(
                "local_provider_runtime_identity_required",
            ))
        })?;
        if !valid_sha256(&value) {
            return Err(AgentError::Core(
                trpg_shared_kernel::TrpgError::InvalidConfiguration(
                    "local_provider_runtime_identity_invalid",
                ),
            ));
        }
        value
    } else {
        "development-runtime-unpinned".to_owned()
    };

    let environment = match config.environment {
        Environment::Dev => "dev",
        Environment::Prod => "prod",
    };
    let mut digest = Sha256::new();
    for field in [
        "trpg-http-model-provider-runtime-v1",
        env!("CARGO_PKG_VERSION"),
        config.provider_type.route_name(),
        config.base_url.as_str(),
        environment,
        runtime_pin.as_str(),
    ] {
        digest.update((field.len() as u64).to_be_bytes());
        digest.update(field.as_bytes());
    }
    Ok(format!("sha256:{:x}", digest.finalize()))
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub fn evaluate_cloud_fallback(
    source: &ProviderConfig,
    target: &ProviderConfig,
    route: &ModelRouteSnapshot,
    authorization: Option<CloudEgressAuthorization>,
    context: &[CloudContextFact],
) -> AgentResult<FallbackDecision> {
    validate_provider_config(source)?;
    validate_provider_config(target)?;
    let local_to_cloud = source.provider_type.is_local()
        && target.provider_type == ProviderType::Cloud
        && route.provider_type == target.provider_type
        && route.model_id == target.model_id;
    let authorized_cloud_route = local_to_cloud
        && authorization.as_ref().is_some_and(|authorization| {
            authorization.permits_context(CloudEgressAttempt {
                source_provider: source.provider_id.as_str(),
                target_provider: target.provider_id.as_str(),
                source_endpoint: &source.base_url,
                target_endpoint: &target.base_url,
                model_id: &route.model_id,
                source_credential: &source.credential,
                target_credential: &target.credential,
                fallback_policy: route.fallback_policy,
                privacy_boundary: route.privacy_boundary,
                context,
            })
        });
    if source.provider_type.is_local()
        && target.provider_type == ProviderType::Cloud
        && !authorized_cloud_route
    {
        return Err(AgentError::SilentFallbackForbidden);
    }

    Ok(if authorized_cloud_route {
        FallbackDecision::Allow
    } else {
        FallbackDecision::DenyAndAudit
    })
}

/// The only transport boundary for cloud model calls. Implementations receive
/// the exact endpoint, provider, model, and context bytes already bound to the
/// persisted cloud-egress authorization.
#[async_trait]
pub trait CloudProviderTransport: Send + Sync {
    async fn send_authorized(
        &self,
        target_endpoint: &str,
        target_provider: &trpg_shared_kernel::EntityId,
        model_id: &str,
        credential: &trpg_security_governance::secret::SecretValue,
        context: &[CloudContextFact],
    ) -> AgentResult<()>;
}

pub struct AuditedCloudSend<'a> {
    pub source: &'a ProviderConfig,
    pub target: &'a ProviderConfig,
    pub route: &'a ModelRouteSnapshot,
    pub context: &'a [CloudContextFact],
}

/// Consumes the non-cloneable authorization in the same operation that invokes
/// the provider adapter. A token for equal-sized but different bytes, another
/// endpoint, provider, or model is rejected before the transport is called.
pub async fn send_audited_cloud_request<
    R: trpg_security_governance::secret::SecretResolver,
    L: trpg_security_governance::cloud_egress::CloudEgressLedger,
>(
    request: AuditedCloudSend<'_>,
    authorization: CloudEgressAuthorization,
    credential_manager: &trpg_security_governance::secret::SecretManager<R>,
    ledger: &L,
    transport: &impl CloudProviderTransport,
) -> AgentResult<()> {
    if !authorization
        .revalidate_for_send(ledger)
        .await
        .map_err(AgentError::Core)?
    {
        return Err(AgentError::SilentFallbackForbidden);
    }
    let decision = evaluate_cloud_fallback(
        request.source,
        request.target,
        request.route,
        Some(authorization),
        request.context,
    )?;
    if decision != FallbackDecision::Allow {
        return Err(AgentError::SilentFallbackForbidden);
    }
    // Both references must still be active at the actual transport boundary.
    // The target credential is exposed only for the lifetime of this exact
    // audited send and never becomes part of ProviderConfig or Debug output.
    let _source_credential = credential_manager
        .resolve(&request.source.credential)
        .map_err(AgentError::Core)?;
    let target_credential = credential_manager
        .resolve(&request.target.credential)
        .map_err(AgentError::Core)?;
    transport
        .send_authorized(
            &request.target.base_url,
            &request.target.provider_id,
            &request.route.model_id,
            &target_credential,
            request.context,
        )
        .await
}

include!("model_provider_sections/01_request_contracts.rs");
include!("model_provider_sections/02_execution_contracts.rs");
