use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use trpg_data_eventing::event_store_sqlx_outbox_projection::PostgresCanonicalStore;
use trpg_identity::WorkloadRole;
use trpg_runtime::runtime;
use trpg_runtime::runtime_state_machines::{
    RuntimeAgent, RuntimeDecision, RuntimeError, RuntimeTool, ToolRequest,
};
use trpg_security_governance::policy_adapter::{
    HttpPolicyEndpoint, OpenFgaOpaPolicyAdapter, PolicyBackend,
};
use trpg_security_governance::tamper_evident_audit::FileAuditLog;
use trpg_shared_kernel::{ActorRole, AuthorityMode, EntityId, TrpgError};

use super::ApiApplication;

const CANONICAL_KEY: [u8; 32] = [0xa7; 32];

struct RealEnvironment {
    primary_url: String,
    witness_url: String,
    openfga_address: SocketAddr,
    openfga_store_id: String,
    openfga_model_id: String,
    opa_address: SocketAddr,
    opa_revision: String,
}

impl RealEnvironment {
    fn load() -> Option<Self> {
        Some(Self {
            primary_url: env::var("P02_FORMAL_COMMIT_DATABASE_URL").ok()?,
            witness_url: env::var("P02_FORMAL_COMMIT_WITNESS_DATABASE_URL").ok()?,
            openfga_address: env::var("P02_OPENFGA_ADDRESS").ok()?.parse().ok()?,
            openfga_store_id: env::var("P02_OPENFGA_STORE_ID").ok()?,
            openfga_model_id: env::var("P02_OPENFGA_MODEL_ID").ok()?,
            opa_address: env::var("P02_OPA_ADDRESS").ok()?.parse().ok()?,
            opa_revision: env::var("P02_OPA_REVISION").ok()?,
        })
    }

    fn policy(&self) -> OpenFgaOpaPolicyAdapter {
        OpenFgaOpaPolicyAdapter::new(
            HttpPolicyEndpoint::new(
                self.openfga_address,
                format!("/stores/{}/check", self.openfga_store_id),
                PolicyBackend::OpenFga,
                self.openfga_model_id.clone(),
            )
            .unwrap(),
            HttpPolicyEndpoint::new(
                self.opa_address,
                "/v1/data/security_governance/decision",
                PolicyBackend::Opa,
                self.opa_revision.clone(),
            )
            .unwrap(),
        )
        .unwrap()
    }
}

fn audit_path(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    env::temp_dir().join(format!(
        "p02-production-custody-{label}-{}-{nonce}.jsonl",
        std::process::id()
    ))
}

fn production_application(
    environment: &RealEnvironment,
    contract: &trpg_shared_kernel::AuthorityContract,
    audit_label: &str,
) -> (ApiApplication, trpg_identity::AuthenticationContext) {
    let identity = trpg_test_support::identity_service_for_contract(contract);
    let credential = identity
        .issue_workload_credential("workflow_001", WorkloadRole::WorkflowEngine, 1, u64::MAX)
        .unwrap();
    let authentication = identity.authenticate_workload(&credential, 2).unwrap();
    let canonical_runtime = tokio::runtime::Runtime::new().unwrap();
    let canonical_store = canonical_runtime
        .block_on(PostgresCanonicalStore::connect(
            &environment.primary_url,
            &environment.witness_url,
            "p02-production-custody-v1",
            &CANONICAL_KEY,
        ))
        .unwrap();
    canonical_runtime
        .block_on(canonical_store.prepare_for_service())
        .unwrap();
    let audit = FileAuditLog::open(
        audit_path(audit_label),
        "p02-production-custody-audit-v1",
        &[0xb8; 32],
    )
    .unwrap();
    (
        ApiApplication::new_production_governed(
            identity,
            environment.policy(),
            audit,
            canonical_runtime,
            canonical_store,
        ),
        authentication,
    )
}

fn campaign_version(application: &ApiApplication, campaign_id: &str) -> u64 {
    let custody = application.canonical_custody.as_ref().unwrap();
    let runtime = custody.runtime.lock().unwrap();
    runtime
        .block_on(custody.store.load_replay_page(campaign_id, 0, 500))
        .unwrap()
        .last()
        .map(|event| u64::try_from(event.stream_version).unwrap())
        .unwrap_or(0)
}

fn replay(application: &ApiApplication, campaign_id: &str) -> Vec<String> {
    let custody = application.canonical_custody.as_ref().unwrap();
    let runtime = custody.runtime.lock().unwrap();
    runtime
        .block_on(custody.store.load_replay_page(campaign_id, 0, 500))
        .unwrap()
        .into_iter()
        .map(|event| event.event_type)
        .collect()
}

fn verify_integrity(application: &ApiApplication) {
    let custody = application.canonical_custody.as_ref().unwrap();
    custody
        .runtime
        .lock()
        .unwrap()
        .block_on(custody.store.verify_integrity())
        .unwrap();
}

#[test]
fn production_runtime_commit_reaches_atomic_canonical_store_and_external_witness() {
    let Some(environment) = RealEnvironment::load() else {
        eprintln!(
            "skipped: configure P02 formal-commit PostgreSQL, OpenFGA, and OPA integration services"
        );
        return;
    };
    let contract =
        trpg_test_support::authority_contract("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    let (mut application, authentication) =
        production_application(&environment, &contract, "success");

    let version_before = campaign_version(&application, contract.campaign_id().as_str());
    let suffix = format!("{}_{version_before}", std::process::id());
    let decision = RuntimeDecision::new(
        format!("decision_production_{suffix}"),
        "production composition root canonical commit",
        ToolRequest::formal(
            RuntimeAgent::AiKeeperOrchestrator,
            RuntimeTool::RequestSkillCheck,
        ),
    )
    .unwrap();
    let mut command = trpg_test_support::governed_command_for_contract(
        &contract,
        decision.clone(),
        ActorRole::Workflow,
    );
    command.command_id = EntityId::new(format!("command_production_{suffix}")).unwrap();
    command.idempotency_key = format!("idempotency_production_{suffix}");
    command.expected_version = version_before;

    let custody = Arc::get_mut(application.canonical_custody.as_mut().unwrap()).unwrap();
    let committed = runtime::commit_runtime_decision(
        &mut custody.runtime_events,
        &contract,
        &command,
        &authentication,
        decision,
        2,
    )
    .unwrap();
    assert_eq!(committed.len(), 2);
    assert_eq!(committed[0].event_type, "ToolRequestApproved");
    assert_eq!(committed[1].event_type, "DecisionCommitted");

    let events = replay(&application, contract.campaign_id().as_str());
    assert_eq!(events.len() as u64, version_before + 2);
    assert_eq!(
        &events[events.len() - 2..],
        &["ToolRequestApproved", "DecisionCommitted"]
    );
    verify_integrity(&application);

    // A fresh composition root has no in-memory version to protect it. The
    // canonical PostgreSQL stream must still reject a stale command, and the
    // failed durable commit must not publish candidate events in memory.
    let (mut restarted, restarted_authentication) =
        production_application(&environment, &contract, "restart-conflict");
    let conflicting_decision = RuntimeDecision::new(
        format!("decision_conflict_{suffix}"),
        "stale restart command",
        ToolRequest::formal(
            RuntimeAgent::AiKeeperOrchestrator,
            RuntimeTool::RequestSkillCheck,
        ),
    )
    .unwrap();
    let mut conflicting_command = trpg_test_support::governed_command_for_contract(
        &contract,
        conflicting_decision.clone(),
        ActorRole::Workflow,
    );
    conflicting_command.command_id = EntityId::new(format!("command_conflict_{suffix}")).unwrap();
    conflicting_command.idempotency_key = format!("idempotency_conflict_{suffix}");
    conflicting_command.expected_version = version_before;

    let restarted_custody = Arc::get_mut(restarted.canonical_custody.as_mut().unwrap()).unwrap();
    let error = runtime::commit_runtime_decision(
        &mut restarted_custody.runtime_events,
        &contract,
        &conflicting_command,
        &restarted_authentication,
        conflicting_decision,
        2,
    )
    .unwrap_err();
    assert_eq!(
        error,
        RuntimeError::Core(TrpgError::ExpectedVersionConflict {
            expected: version_before,
            actual: version_before + 2,
        })
    );
    assert!(restarted_custody.runtime_events.events().is_empty());
    assert_eq!(
        replay(&restarted, contract.campaign_id().as_str()).len() as u64,
        version_before + 2
    );
    verify_integrity(&restarted);
}
