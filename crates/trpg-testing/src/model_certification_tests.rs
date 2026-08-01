use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::{
    evaluate_testing_quality, standard_contract, TestingQualityAction, TestingQualityCommand,
    TestingQualityEventEnvelope, TestingQualityModuleContract, TestingQualityRepository,
};
use trpg_agent_runtime::{
    certify_local_model, evaluate_cloud_fallback, CertificationInput, Environment,
    LocalModelCertificationAuthority, LocalModelLevel, ModelRouteSnapshot, ProviderConfig,
    ProviderType,
};
use trpg_domain_core::command_cqrs::{CommandAcceptedPayload, DomainCommandKind};
use trpg_domain_core::ddd::{
    ActorRole, AuthorityMode, EventStore, FactProvenance, FactSource, ProvenanceKind,
};
use trpg_domain_core::visibility_fact_provenance::CommittedFactEvidence;
use trpg_security_governance::cloud_egress::{
    authorize_cloud_egress, CloudConsentQuery, CloudContextFact, CloudEgressAuditRecord,
    CloudEgressLedger, CloudEgressOutcome, CloudEgressRequest, CloudRouteSnapshotRecord,
    ConsentVisibilityScope, PersistedCloudConsent, ProviderBoundary,
};
use trpg_security_governance::secret::{
    LedgerCheckpoint, LedgerCheckpointStore, SecretReference, LEDGER_CHECKPOINT_GENESIS_HASH,
};
use trpg_shared_kernel::{
    CommandEnvelope, EntityId, KernelResult, PrincipalScope, TrpgError, Visibility, VisibilityLabel,
};

pub const PROMPT_ID: &str = "CODEX-0091-10-TESTING-QUALITY-6730499fe0";
pub const MODULE: &str = "testing_quality::model_certification_tests";

#[derive(Default)]
struct TestingCheckpointStore(Mutex<HashMap<String, Vec<LedgerCheckpoint>>>);

impl LedgerCheckpointStore for TestingCheckpointStore {
    fn latest(&self, ledger_id: &str) -> KernelResult<Option<LedgerCheckpoint>> {
        Ok(self
            .0
            .lock()
            .map_err(|_| TrpgError::AuditIntegrityViolation)?
            .get(ledger_id)
            .and_then(|records| records.last())
            .cloned())
    }

    fn append(&self, ledger_id: &str, checkpoint: &LedgerCheckpoint) -> KernelResult<()> {
        let mut ledgers = self
            .0
            .lock()
            .map_err(|_| TrpgError::AuditIntegrityViolation)?;
        let records = ledgers.entry(ledger_id.to_owned()).or_default();
        if records.last() == Some(checkpoint) {
            return Ok(());
        }
        let valid_predecessor = records.last().map_or(
            checkpoint.sequence() == 1
                && checkpoint.previous_chain_head() == LEDGER_CHECKPOINT_GENESIS_HASH,
            |previous| {
                previous
                    .sequence()
                    .checked_add(1)
                    .is_some_and(|sequence| sequence == checkpoint.sequence())
                    && previous.chain_head() == checkpoint.previous_chain_head()
            },
        );
        if !valid_predecessor {
            return Err(TrpgError::AuditIntegrityViolation);
        }
        records.push(checkpoint.clone());
        Ok(())
    }
}

pub fn contract() -> TestingQualityModuleContract {
    standard_contract(
        PROMPT_ID,
        MODULE,
        "crates/trpg-testing/src/model_certification_tests.rs",
        "crates/trpg-testing/tests/model_certification_tests_contract_tests.rs",
        TestingQualityAction::VerifyModelCertification,
        &[
            "test-data/provider_model_certification_cases.md",
            "fixtures/stages/detailed/S11_golden_visibility_export_diff_expected.current.json.md",
        ],
        &[
            "local_model_level_4_required_for_ai_keeper",
            "silent_local_to_cloud_fallback_denied",
            "provider_boundary_uses_agent_gateway",
        ],
    )
}

pub fn certified_local_model() -> CertificationInput {
    CertificationInput {
        model_id: "json-tool-stable".to_owned(),
        json_schema_support: true,
        tool_call_support: true,
        visibility_tests_pass: true,
        prompt_injection_tests_pass: true,
        rules_eval_pass: true,
        latency_ms: 1_800,
    }
}

pub fn uncertified_local_model() -> CertificationInput {
    CertificationInput {
        model_id: "unstable-chat".to_owned(),
        json_schema_support: false,
        tool_call_support: false,
        visibility_tests_pass: false,
        prompt_injection_tests_pass: false,
        rules_eval_pass: false,
        latency_ms: 5_000,
    }
}

#[allow(deprecated)]
pub fn level4_is_required_for_ai_keeper() -> bool {
    let level = certify_local_model(&certified_local_model());
    let weak_level = certify_local_model(&uncertified_local_model());
    let registry_root = std::env::temp_dir().join(format!(
        "trpg-testing-model-certification-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&registry_root);
    std::fs::create_dir_all(&registry_root).ok();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&registry_root, std::fs::Permissions::from_mode(0o700)).ok();
    }
    let registry_path = registry_root.join("registry.jsonl");
    let result = (|| {
        let authority = LocalModelCertificationAuthority::new_with_checkpoint(
            "testing-certification-key",
            &[0x72; 32],
            &registry_path,
            Arc::new(TestingCheckpointStore::default()),
        )
        .ok()?;
        let artifact = format!("sha256:{}", "1".repeat(64));
        Some(
            level == LocalModelLevel::Level4
                && weak_level != LocalModelLevel::Level4
                && authority
                    .issue_level4(
                        &certified_local_model(),
                        &artifact,
                        "p05-level4-suite-v1",
                        std::time::Duration::from_secs(60),
                    )
                    .is_err()
                && authority
                    .issue_level4(
                        &uncertified_local_model(),
                        &artifact,
                        "p05-level4-suite-v1",
                        std::time::Duration::from_secs(60),
                    )
                    .is_err(),
        )
    })()
    .unwrap_or(false);
    let _ = std::fs::remove_dir_all(registry_root);
    result
}

pub fn silent_cloud_fallback_is_denied() -> bool {
    evaluate_cloud_fallback(
        &testing_local_provider(),
        &testing_cloud_provider(),
        &testing_cloud_route(),
        None,
        &[],
    )
    .is_err()
}

fn testing_local_provider() -> ProviderConfig {
    ProviderConfig {
        provider_id: EntityId::new("ollama").unwrap(),
        provider_type: ProviderType::Ollama,
        model_id: "json-tool-stable".to_owned(),
        model_artifact_sha256: format!("sha256:{}", "1".repeat(64)),
        base_url: "http://127.0.0.1:11434/v1".to_owned(),
        credential: SecretReference::development("testing_ollama", 1).unwrap(),
        environment: Environment::Dev,
    }
}

fn testing_cloud_provider() -> ProviderConfig {
    ProviderConfig {
        provider_id: EntityId::new("cloud").unwrap(),
        provider_type: ProviderType::Cloud,
        model_id: "cloud-model-v1".to_owned(),
        model_artifact_sha256: format!("sha256:{}", "2".repeat(64)),
        base_url: "https://cloud.example.test/v1".to_owned(),
        credential: SecretReference::development("testing_cloud", 1).unwrap(),
        environment: Environment::Dev,
    }
}

fn testing_cloud_route() -> ModelRouteSnapshot {
    ModelRouteSnapshot {
        provider_type: ProviderType::Cloud,
        model_id: "cloud-model-v1".to_owned(),
        fallback_policy: "explicit_audited_only",
        privacy_boundary: "explicit_consent_no_silent_fallback",
    }
}

struct TestingConsentLedger;

#[async_trait]
impl CloudEgressLedger for TestingConsentLedger {
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
            EntityId::new("testing-consent").unwrap(),
            EntityId::new("testing-player").unwrap(),
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

pub async fn explicit_cloud_fallback_is_allowed() -> bool {
    let mut command = trpg_test_support::governed_command(
        "testing cloud context",
        ActorRole::RulesEngine,
        AuthorityMode::HumanKp,
    );
    command.visibility = Visibility::new(VisibilityLabel::Public);
    command.fact_provenance = FactProvenance::new(
        ProvenanceKind::RulesEngineDecision,
        "decision_testing_public_fact",
        "rules_engine_testing",
    )
    .unwrap();
    let mut store = EventStore::default();
    let event = store
        .append(
            &command,
            "DecisionCommitted",
            CommandAcceptedPayload {
                kind: DomainCommandKind::RecordDecision,
                fact_source: FactSource::DecisionRecord,
                target_fact_id: "testing-public-fact".to_owned(),
            },
        )
        .unwrap();
    let evidence =
        CommittedFactEvidence::load(&store, event.sequence, "testing-public-fact").unwrap();
    let context = vec![CloudContextFact::from_committed_fact(&evidence, vec![b'x'; 64]).unwrap()];
    let outcome = authorize_cloud_egress(
        &TestingConsentLedger,
        CloudEgressRequest {
            snapshot_id: EntityId::new("testing-route").unwrap(),
            audit_id: EntityId::new("testing-audit").unwrap(),
            subject_id: EntityId::new("testing-player").unwrap(),
            source_provider: EntityId::new("ollama").unwrap(),
            target_provider: EntityId::new("cloud").unwrap(),
            source_endpoint: "http://127.0.0.1:11434/v1".to_owned(),
            target_endpoint: "https://cloud.example.test/v1".to_owned(),
            model_id: EntityId::new("cloud-model-v1").unwrap(),
            source_credential: SecretReference::development("testing_ollama", 1).unwrap(),
            target_credential: SecretReference::development("testing_cloud", 1).unwrap(),
            source_boundary: ProviderBoundary::Local,
            target_boundary: ProviderBoundary::Cloud,
            fallback_policy: EntityId::new("explicit_audited_only").unwrap(),
            privacy_boundary: EntityId::new("explicit_consent_no_silent_fallback").unwrap(),
            purpose: EntityId::new("gameplay").unwrap(),
            policy_version: EntityId::new("privacy-v1").unwrap(),
            notice_reference: Some(EntityId::new("testing-notice").unwrap()),
            target_audience: PrincipalScope::Player(EntityId::new("testing-player").unwrap()),
            context: context.clone(),
        },
    )
    .await
    .unwrap();
    let CloudEgressOutcome::Authorized(authorization) = outcome else {
        return false;
    };
    evaluate_cloud_fallback(
        &testing_local_provider(),
        &testing_cloud_provider(),
        &testing_cloud_route(),
        Some(*authorization),
        &context,
    )
    .is_ok()
}

pub fn evaluate(
    repository: &mut TestingQualityRepository,
    command: &CommandEnvelope<TestingQualityCommand>,
) -> KernelResult<TestingQualityEventEnvelope> {
    evaluate_testing_quality(MODULE, repository, command)
}
