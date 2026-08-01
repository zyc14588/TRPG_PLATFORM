pub mod common;

use std::sync::Mutex;

use async_trait::async_trait;
use trpg_agent_runtime::model_provider::{
    send_audited_cloud_request, AuditedCloudSend, CloudContextFact, CloudProviderTransport,
    Environment, FallbackDecision, ModelRouteSnapshot, ProviderConfig, ProviderType,
    SecretReference,
};
use trpg_agent_runtime::model_provider_local_cloud_impl;
use trpg_agent_runtime::{AgentResult, EntityId};
use trpg_security_governance::secret::{KmsClient, KmsSecretResolver, SecretManager};
use trpg_shared_kernel::{Visibility, VisibilityLabel};

type CapturedCloudSend = (String, String, String, Vec<Vec<u8>>);

#[derive(Default)]
struct CapturingCloudTransport {
    sent: Mutex<Vec<CapturedCloudSend>>,
}

#[async_trait]
impl CloudProviderTransport for CapturingCloudTransport {
    async fn send_authorized(
        &self,
        target_endpoint: &str,
        target_provider: &EntityId,
        model_id: &str,
        credential: &trpg_security_governance::secret::SecretValue,
        context: &[CloudContextFact],
    ) -> AgentResult<()> {
        credential.expose_to(|bytes| assert_eq!(bytes, b"test-provider-credential"));
        let bytes = context
            .iter()
            .map(|fact| fact.expose_serialized_to(<[u8]>::to_vec))
            .collect();
        self.sent.lock().unwrap().push((
            target_endpoint.to_owned(),
            target_provider.to_string(),
            model_id.to_owned(),
            bytes,
        ));
        Ok(())
    }
}

struct ProviderKms;

impl KmsClient for ProviderKms {
    fn decrypt_secret(
        &self,
        _secret_id: &str,
        _version: u64,
    ) -> trpg_shared_kernel::KernelResult<Vec<u8>> {
        Ok(b"test-provider-credential".to_vec())
    }
}

fn active_configs() -> (
    ProviderConfig,
    ProviderConfig,
    SecretManager<KmsSecretResolver<ProviderKms>>,
) {
    let mut local = local_dev_config();
    let mut cloud = cloud_dev_config();
    local.credential = SecretReference::kms("ollama_active", 1).unwrap();
    cloud.credential = SecretReference::kms("cloud_active", 1).unwrap();
    let manager = SecretManager::new(KmsSecretResolver::new(ProviderKms));
    manager.register(&local.credential).unwrap();
    manager.register(&cloud.credential).unwrap();
    (local, cloud, manager)
}

fn local_dev_config() -> ProviderConfig {
    ProviderConfig {
        provider_id: EntityId::new("ollama").unwrap(),
        provider_type: ProviderType::Ollama,
        model_id: "local-model".to_owned(),
        model_artifact_sha256: format!("sha256:{}", "1".repeat(64)),
        base_url: "http://127.0.0.1:11434/v1".to_owned(),
        credential: SecretReference::development("ollama_dev", 1).unwrap(),
        environment: Environment::Dev,
    }
}

fn cloud_dev_config() -> ProviderConfig {
    ProviderConfig {
        provider_id: EntityId::new("cloud").unwrap(),
        provider_type: ProviderType::Cloud,
        model_id: "cloud-model-v1".to_owned(),
        model_artifact_sha256: format!("sha256:{}", "2".repeat(64)),
        base_url: "https://cloud.example.test/v1".to_owned(),
        credential: SecretReference::development("cloud_dev", 1).unwrap(),
        environment: Environment::Dev,
    }
}

fn route(provider_type: ProviderType, model_id: &str) -> ModelRouteSnapshot {
    ModelRouteSnapshot {
        provider_type,
        model_id: model_id.to_owned(),
        fallback_policy: "explicit_audited_only",
        privacy_boundary: "explicit_consent_no_silent_fallback",
    }
}

#[test]
fn model_provider_local_cloud_impl_requires_level4_for_ai_keeper() {
    assert_eq!(
        trpg_test_support::normalized_prompt_id(
            "trpg-agent-runtime",
            "model_provider_local_cloud_impl"
        ),
        "CODEX-0484-04-AI-AGENT-SYSTEM-e96dc3868d"
    );
    let local = local_dev_config();
    let fixture = common::level4_certification_for_provider(&local);
    fixture.authority.revoke(&fixture.certificate).unwrap();
    let error = model_provider_local_cloud_impl::evaluate_provider_route_for_ai_keeper(
        &local,
        &local,
        &route(ProviderType::Ollama, "local-model"),
        None,
        &[],
        &fixture.authority,
        &fixture.certificate,
    )
    .unwrap_err();

    assert_eq!(error.code(), "LOCAL_MODEL_NOT_CERTIFIED_FOR_AI_KP");
}

#[test]
fn ai_keeper_certificate_is_bound_to_the_resolved_provider_runtime() {
    let local = local_dev_config();
    let fixture = common::level4_certification_for_provider(&local);
    fixture
        .authority
        .ensure_ai_keeper_provider_config(&fixture.certificate, &local)
        .unwrap();

    let mut drifted_runtime = local;
    drifted_runtime.base_url = "http://127.0.0.1:11435/v1".to_owned();
    assert!(fixture
        .authority
        .ensure_ai_keeper_provider_config(&fixture.certificate, &drifted_runtime)
        .is_err());
}

#[test]
fn model_provider_local_cloud_impl_blocks_silent_local_to_cloud_fallback() {
    let local = local_dev_config();
    let fixture = common::level4_certification_for_provider(&local);
    let error = model_provider_local_cloud_impl::evaluate_provider_route_for_ai_keeper(
        &local,
        &cloud_dev_config(),
        &route(ProviderType::Cloud, "cloud-model-v1"),
        None,
        &[],
        &fixture.authority,
        &fixture.certificate,
    )
    .unwrap_err();

    assert_eq!(error.code(), "SILENT_FALLBACK_FORBIDDEN");
}

#[tokio::test]
async fn model_provider_local_cloud_impl_accepts_explicit_audited_route() {
    let authorization = common::cloud_egress_authorization().await;
    let context = common::cloud_egress_context();
    let local = local_dev_config();
    let fixture = common::level4_certification_for_provider(&local);
    let evaluation = model_provider_local_cloud_impl::evaluate_provider_route_for_ai_keeper(
        &local,
        &cloud_dev_config(),
        &route(ProviderType::Cloud, "cloud-model-v1"),
        Some(authorization),
        &context,
        &fixture.authority,
        &fixture.certificate,
    )
    .unwrap();

    assert_eq!(evaluation.fallback, FallbackDecision::Allow);
    assert!(evaluation.ai_keeper_allowed);
    assert_eq!(evaluation.boundary.gateway, "Agent Gateway");
    assert_eq!(
        evaluation.boundary.provider_adapter,
        "Model Provider Adapter"
    );
}

#[tokio::test]
async fn audited_route_cannot_be_reused_for_a_different_context() {
    let authorization = common::cloud_egress_authorization().await;
    let local = local_dev_config();
    let fixture = common::level4_certification_for_provider(&local);
    let different_context = vec![common::verified_cloud_fact(
        "different-public-fact",
        Visibility::new(VisibilityLabel::Public),
        vec![b'y'; 64],
    )];
    let error = model_provider_local_cloud_impl::evaluate_provider_route_for_ai_keeper(
        &local,
        &cloud_dev_config(),
        &route(ProviderType::Cloud, "cloud-model-v1"),
        Some(authorization),
        &different_context,
        &fixture.authority,
        &fixture.certificate,
    )
    .unwrap_err();

    assert_eq!(error.code(), "SILENT_FALLBACK_FORBIDDEN");
}

#[tokio::test]
async fn provider_send_consumes_authorization_for_exact_endpoint_model_and_bytes() {
    let transport = CapturingCloudTransport::default();
    let context = common::cloud_egress_context();
    let (local, cloud, credentials) = active_configs();
    let send_route = route(ProviderType::Cloud, "cloud-model-v1");
    send_audited_cloud_request(
        AuditedCloudSend {
            source: &local,
            target: &cloud,
            route: &send_route,
            context: &context,
        },
        common::cloud_egress_authorization_for(local.credential.clone(), cloud.credential.clone())
            .await,
        &credentials,
        &common::PersistedConsentLedger,
        &transport,
    )
    .await
    .unwrap();
    let sent = transport.sent.lock().unwrap();
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0, "https://cloud.example.test/v1");
    assert_eq!(sent[0].1, "cloud");
    assert_eq!(sent[0].2, "cloud-model-v1");
    assert_eq!(sent[0].3, vec![vec![b'x'; 64]]);
}

#[tokio::test]
async fn provider_send_rejects_same_size_different_bytes_and_route_changes() {
    let transport = CapturingCloudTransport::default();
    let (local, cloud, credentials) = active_configs();
    let changed_bytes = vec![common::verified_cloud_fact(
        "agent-test-public-fact",
        Visibility::new(VisibilityLabel::Public),
        vec![b'z'; 64],
    )];
    let changed_bytes_route = route(ProviderType::Cloud, "cloud-model-v1");
    assert!(send_audited_cloud_request(
        AuditedCloudSend {
            source: &local,
            target: &cloud,
            route: &changed_bytes_route,
            context: &changed_bytes,
        },
        common::cloud_egress_authorization_for(local.credential.clone(), cloud.credential.clone(),)
            .await,
        &credentials,
        &common::PersistedConsentLedger,
        &transport,
    )
    .await
    .is_err());

    let mut changed_endpoint = cloud.clone();
    changed_endpoint.base_url = "https://other-cloud.example.test/v1".to_owned();
    let changed_endpoint_route = route(ProviderType::Cloud, "cloud-model-v1");
    let changed_endpoint_context = common::cloud_egress_context();
    assert!(send_audited_cloud_request(
        AuditedCloudSend {
            source: &local,
            target: &changed_endpoint,
            route: &changed_endpoint_route,
            context: &changed_endpoint_context,
        },
        common::cloud_egress_authorization_for(
            local.credential.clone(),
            changed_endpoint.credential.clone(),
        )
        .await,
        &credentials,
        &common::PersistedConsentLedger,
        &transport,
    )
    .await
    .is_err());

    assert!(transport.sent.lock().unwrap().is_empty());
}

#[tokio::test]
async fn provider_send_rechecks_secret_lifecycle_at_the_transport_boundary() {
    let transport = CapturingCloudTransport::default();
    let (local, cloud, credentials) = active_configs();
    credentials.revoke(&cloud.credential).unwrap();

    let send_route = route(ProviderType::Cloud, "cloud-model-v1");
    let context = common::cloud_egress_context();
    assert!(send_audited_cloud_request(
        AuditedCloudSend {
            source: &local,
            target: &cloud,
            route: &send_route,
            context: &context,
        },
        common::cloud_egress_authorization_for(local.credential.clone(), cloud.credential.clone(),)
            .await,
        &credentials,
        &common::PersistedConsentLedger,
        &transport,
    )
    .await
    .is_err());
    assert!(transport.sent.lock().unwrap().is_empty());
}
