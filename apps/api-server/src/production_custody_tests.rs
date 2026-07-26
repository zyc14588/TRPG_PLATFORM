use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use trpg_contracts::HttpRequest;
use trpg_data_eventing::event_store_sqlx_outbox_projection::PostgresCanonicalStore;
use trpg_identity::{GlobalRole, WorkloadRole};
use trpg_runtime::runtime;
use trpg_runtime::runtime_state_machines::{
    RuntimeAgent, RuntimeDecision, RuntimeError, RuntimeTool, ToolRequest,
};
use trpg_security_governance::policy_adapter::{
    HttpPolicyEndpoint, OpenFgaOpaPolicyAdapter, PolicyBackend,
};
use trpg_security_governance::security_privacy::PostgresDeletionRepository;
use trpg_security_governance::tamper_evident_audit::FileAuditLog;
use trpg_shared_kernel::{ActorRole, AuthorityMode, EntityId, TrpgError};

use super::{build_kernel_error_response, ApiApplication};

const CANONICAL_KEY: [u8; 32] = [0xa7; 32];
const PAYLOAD_KEY: [u8; 32] = [0xb8; 32];

#[test]
fn production_error_boundary_redacts_restricted_root_cause_and_keeps_correlation() {
    let response = build_kernel_error_response(
        &TrpgError::PolicyUnavailable,
        "canonical_replay",
        "campaign_error_boundary",
        "correlation_error_boundary",
        "trace_error_boundary",
        "database password and keeper secret must remain internal",
    );
    let body = response.body.clone();
    assert_eq!(response.status, 503);
    assert_eq!(body["code"], "INTERNAL_ERROR");
    assert_eq!(body["correlation_id"], "correlation_error_boundary");
    let serialized = serde_json::to_string(&response.body).unwrap();
    assert!(!serialized.contains("database password"));
    assert!(!serialized.contains("keeper secret"));
}

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
    fn load() -> Self {
        let endpoints = trpg_test_support::formal_commit_policy_endpoints();
        Self {
            primary_url: env::var("P02_FORMAL_COMMIT_DATABASE_URL").expect(
                "P02_FORMAL_COMMIT_DATABASE_URL is required for the production custody gate",
            ),
            witness_url: env::var("P02_FORMAL_COMMIT_WITNESS_DATABASE_URL").expect(
                "P02_FORMAL_COMMIT_WITNESS_DATABASE_URL is required for the production custody gate",
            ),
            openfga_address: endpoints.openfga,
            openfga_store_id: "test".to_owned(),
            openfga_model_id: endpoints.openfga_model.to_owned(),
            opa_address: endpoints.opa,
            opa_revision: endpoints.opa_revision.to_owned(),
        }
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

fn api_request(
    method: &str,
    path: &str,
    token: Option<&str>,
    body: serde_json::Value,
) -> HttpRequest {
    let mut headers = HashMap::new();
    headers.insert("content-type".to_owned(), "application/json".to_owned());
    headers.insert(
        "idempotency-key".to_owned(),
        "privacy_api_idem_001".to_owned(),
    );
    headers.insert("x-trace-id".to_owned(), "privacy_api_trace_001".to_owned());
    if let Some(token) = token {
        headers.insert("authorization".to_owned(), format!("Bearer {token}"));
    }
    HttpRequest {
        method: method.to_owned(),
        path: path.to_owned(),
        headers,
        body: serde_json::to_vec(&body).unwrap(),
    }
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
            "p05-production-payload-v1",
            &PAYLOAD_KEY,
        ))
        .unwrap();
    canonical_runtime
        .block_on(canonical_store.prepare_for_service())
        .unwrap();
    let privacy_runtime = tokio::runtime::Runtime::new().unwrap();
    let deletion_repository = privacy_runtime
        .block_on(PostgresDeletionRepository::connect(
            &environment.primary_url,
        ))
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
            privacy_runtime,
            deletion_repository,
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
fn production_privacy_api_binds_job_to_real_canonical_event_and_protects_status() {
    let environment = RealEnvironment::load();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let campaign_id = format!("campaign_privacy_api_{nonce}");
    let contract =
        trpg_test_support::authority_contract(&campaign_id, AuthorityMode::HumanKp, 1).unwrap();
    let subject_id = contract.authority_owner().as_str().to_owned();
    let subject_login = if subject_id == "test_authority_registrar" {
        "test-authority-registrar@example.test".to_owned()
    } else {
        format!("{subject_id}@example.test")
    };
    let (application, _) = production_application(&environment, &contract, "privacy-api");
    let now = u64::try_from(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap();
    let (token, intruder_token) = {
        let identity = application.authentication.identity();
        let mut identity = identity.lock().unwrap();
        let session = identity
            .login(&subject_login, "test authority password long enough", now)
            .unwrap();
        identity
            .create_user(
                format!("privacy_intruder_{nonce}"),
                &format!("privacy-intruder-{nonce}@example.test"),
                "privacy intruder password long enough",
                GlobalRole::User,
            )
            .unwrap();
        let intruder = identity
            .login(
                &format!("privacy-intruder-{nonce}@example.test"),
                "privacy intruder password long enough",
                now,
            )
            .unwrap();
        (
            session.token.expose().to_owned(),
            intruder.token.expose().to_owned(),
        )
    };
    let job_id = format!("privacy_api_job_{nonce}");
    let body = serde_json::json!({
        "job_id": job_id,
        "subject_id": subject_id,
        "retention_policy": "user_erasure_v1",
        "reason": "private user supplied reason",
        "command_id": format!("privacy_api_command_{nonce}"),
        "correlation_id": format!("privacy_api_correlation_{nonce}"),
        "causation_id": format!("privacy_api_causation_{nonce}"),
        "expected_version": 0,
    });
    let unauthenticated = application
        .handle(&api_request(
            "POST",
            &format!("/campaigns/{campaign_id}/privacy/deletions"),
            None,
            body.clone(),
        ))
        .unwrap();
    assert_eq!(unauthenticated.status, 401);

    let response = application
        .handle(&api_request(
            "POST",
            &format!("/campaigns/{campaign_id}/privacy/deletions"),
            Some(&token),
            body,
        ))
        .unwrap();
    assert_eq!(response.status, 202, "response={:?}", response.body);
    assert_eq!(response.body["job_id"], job_id);
    assert_eq!(response.body["evidence_status"], "confirmed");

    let custody = application.canonical_custody.as_ref().unwrap();
    let persisted_job = custody
        .privacy_runtime
        .lock()
        .unwrap()
        .block_on(custody.deletion_repository.load(&job_id))
        .unwrap();
    assert_eq!(
        persisted_job.evidence_status,
        trpg_security_governance::security_privacy::DeletionEvidenceStatus::Confirmed
    );
    assert!(persisted_job
        .canonical_event_integrity_hash
        .as_deref()
        .unwrap()
        .starts_with("hmac-sha256:"));
    let events = custody
        .runtime
        .lock()
        .unwrap()
        .block_on(custody.store.load_replay_page(&campaign_id, 0, 100))
        .unwrap();
    let deletion_event = events
        .iter()
        .find(|event| {
            event.event_type == "platform.security_privacy_copyright.data_deletion_requested"
        })
        .expect("canonical deletion request event");
    assert_eq!(
        deletion_event.event_integrity_hash,
        persisted_job.canonical_event_integrity_hash
    );
    assert_eq!(deletion_event.provenance_kind, "user_statement");
    assert_eq!(
        deletion_event.visibility_subject,
        contract.authority_owner().as_str()
    );
    assert!(!deletion_event
        .payload
        .to_string()
        .contains("private user supplied reason"));

    let status = application
        .handle(&api_request(
            "GET",
            &format!("/campaigns/{campaign_id}/privacy/deletions/{job_id}"),
            Some(&token),
            serde_json::json!({}),
        ))
        .unwrap();
    assert_eq!(status.status, 200);
    assert_eq!(status.body["evidence_status"], "confirmed");
    let forbidden = application
        .handle(&api_request(
            "GET",
            &format!("/campaigns/{campaign_id}/privacy/deletions/{job_id}"),
            Some(&intruder_token),
            serde_json::json!({}),
        ))
        .unwrap();
    assert_eq!(forbidden.status, 404);
}

#[test]
fn production_runtime_without_a_bound_tool_executor_fails_closed() {
    let environment = RealEnvironment::load();
    let contract =
        trpg_test_support::authority_contract("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    let (mut application, authentication) =
        production_application(&environment, &contract, "missing-tool-executor");

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
    let error = runtime::commit_runtime_decision(
        &mut custody.runtime_events,
        &contract,
        &command,
        &authentication,
        decision.clone(),
        2,
    )
    .unwrap_err();
    assert_eq!(error, RuntimeError::AgentToolNotAllowed);
    assert!(custody.runtime_events.events().is_empty());
    assert_eq!(
        replay(&application, contract.campaign_id().as_str()).len() as u64,
        version_before
    );
    verify_integrity(&application);
}
