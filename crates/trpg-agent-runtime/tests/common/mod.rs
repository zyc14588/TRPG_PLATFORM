use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use trpg_domain_core::command_cqrs::{CommandAcceptedPayload, DomainCommandKind};
use trpg_domain_core::ddd::{
    ActorRole as DomainActorRole, AuthorityMode as DomainAuthorityMode,
    EventStore as DomainEventStore, FactProvenance as DomainFactProvenance,
    FactSource as DomainFactSource, ProvenanceKind as DomainProvenanceKind,
};
use trpg_domain_core::visibility_fact_provenance::CommittedFactEvidence;

use trpg_agent_runtime::local_model_certification::{
    CertificationInput, LocalModelCertificate, LocalModelCertificationAuthority,
};
use trpg_agent_runtime::{
    AgentEventPayload, AgentEventStore, AuthorityContract, ContextFact, FormalCommitAudit,
    FormalCommitAuthorizer,
};
use trpg_security_governance::cloud_egress::{
    authorize_cloud_egress, CloudConsentQuery, CloudContextFact, CloudEgressAuditRecord,
    CloudEgressAuthorization, CloudEgressLedger, CloudEgressOutcome, CloudEgressRequest,
    CloudRouteSnapshotRecord, ConsentVisibilityScope, PersistedCloudConsent, ProviderBoundary,
};
use trpg_security_governance::policy_adapter::{
    HttpPolicyEndpoint, OpenFgaOpaPolicyAdapter, PolicyBackend,
};
use trpg_security_governance::secret::SecretReference;
use trpg_shared_kernel::{
    CanonicalCommitPort, EntityId, FactProvenance, KernelResult, PrincipalScope, ProvenanceKind,
    TrpgError, Visibility, VisibilityLabel,
};

static NEXT_AUDIT_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_CERTIFICATION_ID: AtomicU64 = AtomicU64::new(1);

pub struct CertificationFixture {
    pub authority: LocalModelCertificationAuthority,
    pub certificate: LocalModelCertificate,
    registry_path: std::path::PathBuf,
}

impl Drop for CertificationFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.registry_path);
    }
}

pub fn level4_certification(model_id: &str, model_artifact_sha256: &str) -> CertificationFixture {
    let nonce = NEXT_CERTIFICATION_ID.fetch_add(1, Ordering::Relaxed);
    let registry_path = std::env::temp_dir().join(format!(
        "trpg-local-model-certification-{}-{nonce}.jsonl",
        std::process::id()
    ));
    let authority = LocalModelCertificationAuthority::new(
        "test-certification-key",
        &[0x91; 32],
        &registry_path,
    )
    .unwrap();
    let certificate = authority
        .issue_level4(
            &CertificationInput {
                model_id: model_id.to_owned(),
                json_schema_support: true,
                tool_call_support: true,
                visibility_tests_pass: true,
                prompt_injection_tests_pass: true,
                rules_eval_pass: true,
                latency_ms: 500,
            },
            model_artifact_sha256,
            "p05-level4-suite-v1",
            Duration::from_secs(60),
        )
        .unwrap();
    CertificationFixture {
        authority,
        certificate,
        registry_path,
    }
}

pub struct PersistedConsentLedger;

#[async_trait]
impl CloudEgressLedger for PersistedConsentLedger {
    async fn trusted_now_unix_ms(&self) -> KernelResult<u64> {
        Ok(10_000)
    }

    async fn notice_is_recorded(
        &self,
        notice_reference: &EntityId,
        _subject_id: &EntityId,
        _policy_version: &EntityId,
    ) -> KernelResult<bool> {
        Ok(!notice_reference.as_str().is_empty())
    }

    async fn load_active_consent(
        &self,
        _query: &CloudConsentQuery,
    ) -> KernelResult<Option<PersistedCloudConsent>> {
        Ok(Some(PersistedCloudConsent::loaded_from_repository(
            EntityId::new("agent-test-consent").unwrap(),
            EntityId::new("agent-test-player").unwrap(),
            EntityId::new("cloud").unwrap(),
            EntityId::new("gameplay").unwrap(),
            EntityId::new("privacy-v1").unwrap(),
            ConsentVisibilityScope::PublicOnly,
            20_000,
        )))
    }

    async fn record_route_decision(
        &self,
        _snapshot: CloudRouteSnapshotRecord,
        _audit: CloudEgressAuditRecord,
    ) -> KernelResult<bool> {
        Ok(true)
    }
}

pub async fn cloud_egress_authorization() -> CloudEgressAuthorization {
    cloud_egress_authorization_for(
        SecretReference::development("ollama_dev", 1).unwrap(),
        SecretReference::development("cloud_dev", 1).unwrap(),
    )
    .await
}

pub async fn cloud_egress_authorization_for(
    source_credential: SecretReference,
    target_credential: SecretReference,
) -> CloudEgressAuthorization {
    let outcome = authorize_cloud_egress(
        &PersistedConsentLedger,
        CloudEgressRequest {
            snapshot_id: EntityId::new("agent-test-route").unwrap(),
            audit_id: EntityId::new("agent-test-audit").unwrap(),
            subject_id: EntityId::new("agent-test-player").unwrap(),
            source_provider: EntityId::new("ollama").unwrap(),
            target_provider: EntityId::new("cloud").unwrap(),
            source_endpoint: "http://127.0.0.1:11434/v1".to_owned(),
            target_endpoint: "https://cloud.example.test/v1".to_owned(),
            model_id: EntityId::new("cloud-model-v1").unwrap(),
            source_credential,
            target_credential,
            source_boundary: ProviderBoundary::Local,
            target_boundary: ProviderBoundary::Cloud,
            fallback_policy: EntityId::new("explicit_audited_only").unwrap(),
            privacy_boundary: EntityId::new("explicit_consent_no_silent_fallback").unwrap(),
            purpose: EntityId::new("gameplay").unwrap(),
            policy_version: EntityId::new("privacy-v1").unwrap(),
            notice_reference: Some(EntityId::new("agent-test-notice").unwrap()),
            target_audience: PrincipalScope::Player(EntityId::new("agent-test-player").unwrap()),
            context: cloud_egress_context(),
        },
    )
    .await
    .unwrap();
    let CloudEgressOutcome::Authorized(authorization) = outcome else {
        panic!("test consent must create an authorization")
    };
    *authorization
}

pub fn cloud_egress_context() -> Vec<CloudContextFact> {
    vec![verified_cloud_fact(
        "agent-test-public-fact",
        Visibility::new(VisibilityLabel::Public),
        vec![b'x'; 64],
    )]
}

pub fn verified_cloud_fact(
    fact_id: &str,
    visibility: Visibility,
    content: Vec<u8>,
) -> CloudContextFact {
    let mut command = trpg_test_support::governed_command(
        "verified agent cloud context",
        DomainActorRole::RulesEngine,
        DomainAuthorityMode::HumanKp,
    );
    command.visibility = visibility;
    command.fact_provenance = DomainFactProvenance::new(
        DomainProvenanceKind::RulesEngineDecision,
        format!("decision_{fact_id}"),
        "rules_engine_agent_context",
    )
    .unwrap();
    let mut store = DomainEventStore::default();
    let event = store
        .append(
            &command,
            "DecisionCommitted",
            CommandAcceptedPayload {
                kind: DomainCommandKind::RecordDecision,
                fact_source: DomainFactSource::DecisionRecord,
                target_fact_id: fact_id.to_owned(),
            },
        )
        .unwrap();
    let evidence = CommittedFactEvidence::load(&store, event.sequence, fact_id).unwrap();
    CloudContextFact::from_committed_fact(&evidence, content).unwrap()
}

pub fn context_fact(
    fact_id: &str,
    text: &str,
    visibility: Visibility,
) -> Result<ContextFact, TrpgError> {
    let provenance_reference = format!("{fact_id}_source");
    ContextFact::new(
        fact_id,
        text,
        visibility,
        FactProvenance::new(
            ProvenanceKind::ToolResult,
            provenance_reference,
            "context_test_tool",
        )?,
    )
}

pub fn audited_store(contract: &AuthorityContract) -> AgentEventStore<AgentEventPayload> {
    audited_store_with_handle(contract).0
}

pub fn audited_store_with_handle(
    contract: &AuthorityContract,
) -> (AgentEventStore<AgentEventPayload>, FormalCommitAudit) {
    audited_store_with_canonical(contract, trpg_test_support::test_canonical_commit_port())
}

pub fn audited_store_with_canonical(
    contract: &AuthorityContract,
    canonical: Arc<dyn CanonicalCommitPort>,
) -> (AgentEventStore<AgentEventPayload>, FormalCommitAudit) {
    let audit_id = NEXT_AUDIT_ID.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "p02-agent-integration-audit-{}-{audit_id}.jsonl",
        std::process::id()
    ));
    let audit = FormalCommitAudit::open(path, "agent-integration-test-v1", &[0x85; 32]).unwrap();
    let endpoints = trpg_test_support::formal_commit_policy_endpoints();
    let policy = OpenFgaOpaPolicyAdapter::new(
        HttpPolicyEndpoint::new(
            endpoints.openfga,
            "/stores/test/check",
            PolicyBackend::OpenFga,
            endpoints.openfga_model,
        )
        .unwrap(),
        HttpPolicyEndpoint::new(
            endpoints.opa,
            "/v1/data/security_governance/decision",
            PolicyBackend::Opa,
            endpoints.opa_revision,
        )
        .unwrap(),
    )
    .unwrap();
    let (identity_verifier, _) = trpg_test_support::formal_commit_identity_for_contract(contract);
    (
        AgentEventStore::with_formal_custody(
            FormalCommitAuthorizer::new(identity_verifier, policy, audit.clone()),
            canonical,
        ),
        audit,
    )
}
