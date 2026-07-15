use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use trpg_identity::{IdentityService, WorkloadRole};
use trpg_security_governance::formal_commit_audit::{FormalCommitAudit, FormalCommitAuthorizer};
use trpg_security_governance::policy_adapter::{
    HttpPolicyEndpoint, OpenFgaOpaPolicyAdapter, PolicyBackend,
};
use trpg_security_governance::tamper_evident_audit::{
    AuditDecision, AuditRecordDraft, AuditSink, FileAuditLog,
};
use trpg_security_governance::{
    evaluate_security_governance, evaluate_security_governance_with_policy, PolicyIdentityContext,
    SecurityGovernanceAction, SecurityGovernanceCommand, SecurityGovernanceRepository,
};
use trpg_shared_kernel::{ActorRole, AuthorityMode, TrpgError, Visibility, VisibilityLabel};

const AUDIT_KEY: [u8; 32] = [0x42; 32];
const IDENTITY_KEY: [u8; 32] = [0x5a; 32];

fn command(
    action: SecurityGovernanceAction,
) -> trpg_shared_kernel::CommandEnvelope<SecurityGovernanceCommand> {
    trpg_test_support::governed_command(
        SecurityGovernanceCommand::new(action),
        ActorRole::Workflow,
        AuthorityMode::HumanKp,
    )
}

fn audit_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "trpg-policy-{label}-{}-{:?}.jsonl",
        std::process::id(),
        thread::current().id()
    ))
}

fn open_audit(path: &Path) -> FileAuditLog {
    cleanup_audit(path);
    FileAuditLog::open(path, "test-audit-key-v1", &AUDIT_KEY).unwrap()
}

fn open_formal_audit(path: &Path) -> FormalCommitAudit {
    cleanup_audit(path);
    FormalCommitAudit::open(path, "test-formal-audit-key-v1", &AUDIT_KEY).unwrap()
}

fn audit_anchor_path(path: &Path) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".head");
    PathBuf::from(name)
}

fn cleanup_audit(path: &Path) {
    let _ = std::fs::remove_file(path);
    let _ = std::fs::remove_file(audit_anchor_path(path));
}

fn adapter(
    openfga: SocketAddr,
    openfga_path: String,
    openfga_model: String,
    opa: SocketAddr,
    opa_revision: String,
) -> OpenFgaOpaPolicyAdapter {
    OpenFgaOpaPolicyAdapter::new(
        HttpPolicyEndpoint::new(openfga, openfga_path, PolicyBackend::OpenFga, openfga_model)
            .unwrap(),
        HttpPolicyEndpoint::new(
            opa,
            "/v1/data/security_governance/decision",
            PolicyBackend::Opa,
            opa_revision,
        )
        .unwrap(),
    )
    .unwrap()
}

fn one_shot_policy_response(body: &'static str, extra_headers: &'static str) -> SocketAddr {
    one_shot_policy_response_with_capture(body, extra_headers).0
}

fn one_shot_policy_response_with_capture(
    body: &'static str,
    extra_headers: &'static str,
) -> (SocketAddr, Receiver<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let (request_sender, request_receiver) = mpsc::channel();
    thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4096];
        loop {
            let count = stream.read(&mut buffer).unwrap();
            request.extend_from_slice(&buffer[..count]);
            let Some(boundary) = request.windows(4).position(|window| window == b"\r\n\r\n") else {
                continue;
            };
            let headers = String::from_utf8_lossy(&request[..boundary]);
            let content_length = headers
                .lines()
                .filter_map(|line| line.split_once(':'))
                .find(|(name, _)| name.eq_ignore_ascii_case("content-length"))
                .map(|(_, value)| value.trim().parse::<usize>().unwrap())
                .unwrap_or(0);
            if request.len() >= boundary + 4 + content_length {
                break;
            }
        }
        let _ = request_sender.send(request);
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n{extra_headers}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len(),
        )
        .unwrap();
    });
    (address, request_receiver)
}

fn real_policy_adapter() -> Option<OpenFgaOpaPolicyAdapter> {
    let openfga = std::env::var("P02_OPENFGA_ADDRESS")
        .ok()?
        .parse::<SocketAddr>()
        .ok()?;
    let store_id = std::env::var("P02_OPENFGA_STORE_ID").ok()?;
    let model_id = std::env::var("P02_OPENFGA_MODEL_ID").ok()?;
    let opa = std::env::var("P02_OPA_ADDRESS")
        .ok()?
        .parse::<SocketAddr>()
        .ok()?;
    let opa_revision = std::env::var("P02_OPA_REVISION")
        .unwrap_or_else(|_| "opa-security-governance-v2".to_owned());
    Some(adapter(
        openfga,
        format!("/stores/{store_id}/check"),
        model_id,
        opa,
        opa_revision,
    ))
}

fn evaluate_with_trusted_workload(
    module: &'static str,
    repository: &mut SecurityGovernanceRepository,
    command: &trpg_shared_kernel::CommandEnvelope<SecurityGovernanceCommand>,
    policy: &OpenFgaOpaPolicyAdapter,
    audit: &mut FileAuditLog,
) -> trpg_shared_kernel::KernelResult<trpg_security_governance::SecurityGovernanceEventEnvelope> {
    let identity = IdentityService::new(&IDENTITY_KEY, 60_000).unwrap();
    let credential = identity
        .issue_workload_credential("workflow_001", WorkloadRole::WorkflowEngine, 1, 10_000)
        .unwrap();
    let authentication = identity.authenticate_workload(&credential, 2).unwrap();
    let verifier = identity.verifier();
    evaluate_security_governance_with_policy(
        module,
        repository,
        command,
        policy,
        audit,
        PolicyIdentityContext::new(&verifier, &authentication, 2),
    )
}

#[test]
fn no_adapter_and_unreachable_policy_fail_closed_and_unavailable_is_audited() {
    assert_eq!(
        HttpPolicyEndpoint::new(
            "192.0.2.1:8080".parse().unwrap(),
            "/check",
            PolicyBackend::OpenFga,
            "model",
        )
        .unwrap_err(),
        TrpgError::InvalidConfiguration("policy_endpoint_configuration_invalid")
    );
    let command = command(SecurityGovernanceAction::RecordAudit);
    let mut repository = SecurityGovernanceRepository::default();
    assert_eq!(
        evaluate_security_governance("policy_test", &mut repository, &command).unwrap_err(),
        TrpgError::PolicyUnavailable
    );

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let unavailable = listener.local_addr().unwrap();
    drop(listener);
    let policy = adapter(
        unavailable,
        "/stores/unavailable/check".to_owned(),
        "openfga-model-unavailable".to_owned(),
        unavailable,
        "opa-unavailable".to_owned(),
    );
    let path = audit_path("unavailable");
    let mut audit = open_audit(&path);

    assert_eq!(
        evaluate_with_trusted_workload(
            "policy_test",
            &mut repository,
            &command,
            &policy,
            &mut audit,
        )
        .unwrap_err(),
        TrpgError::PolicyUnavailable
    );
    assert!(repository.events().is_empty());
    let records = audit.verify().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].decision, AuditDecision::Unavailable);
    assert_eq!(
        records[0].openfga_policy_revision,
        "openfga-model-unavailable"
    );
    cleanup_audit(&path);
}

#[test]
fn local_permission_denial_is_audited_before_any_external_policy_call() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let unavailable = listener.local_addr().unwrap();
    drop(listener);
    let policy = adapter(
        unavailable,
        "/stores/unavailable/check".to_owned(),
        "openfga-model-unavailable".to_owned(),
        unavailable,
        "opa-unavailable".to_owned(),
    );
    let path = audit_path("local-deny");
    let mut audit = open_audit(&path);
    let mut repository = SecurityGovernanceRepository::default();
    let command = command(SecurityGovernanceAction::OverrideDiceRoll);

    assert_eq!(
        evaluate_with_trusted_workload(
            "policy_test",
            &mut repository,
            &command,
            &policy,
            &mut audit,
        )
        .unwrap_err(),
        TrpgError::PolicyDenied
    );
    assert!(repository.events().is_empty());
    let records = audit.verify().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].decision, AuditDecision::Deny);
    assert_eq!(records[0].openfga_decision_id, "local-permission-deny");
    cleanup_audit(&path);
}

#[test]
fn caller_reported_internal_actor_is_rejected_before_policy_evaluation() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let unavailable = listener.local_addr().unwrap();
    drop(listener);
    let policy = adapter(
        unavailable,
        "/stores/unavailable/check".to_owned(),
        "openfga-model-unavailable".to_owned(),
        unavailable,
        "opa-unavailable".to_owned(),
    );
    let path = audit_path("forged-internal-actor");
    let mut audit = open_audit(&path);
    let forged = trpg_test_support::governed_command(
        SecurityGovernanceCommand::new(SecurityGovernanceAction::RecordAudit),
        ActorRole::RulesEngine,
        AuthorityMode::HumanKp,
    );

    assert_eq!(
        evaluate_with_trusted_workload(
            "policy_test",
            &mut SecurityGovernanceRepository::default(),
            &forged,
            &policy,
            &mut audit,
        ),
        Err(TrpgError::InternalIdentityInvalid)
    );
    assert!(audit.verify().unwrap().is_empty());
    cleanup_audit(&path);
}

#[test]
fn policy_evidence_without_server_decision_id_or_exact_revision_is_rejected() {
    let missing_id = one_shot_policy_response(r#"{"allowed":true}"#, "");
    let unreachable_opa = TcpListener::bind("127.0.0.1:0").unwrap();
    let unreachable_opa_address = unreachable_opa.local_addr().unwrap();
    drop(unreachable_opa);
    let policy = adapter(
        missing_id,
        "/stores/test/check".to_owned(),
        "model-test".to_owned(),
        unreachable_opa_address,
        "opa-security-governance-v2".to_owned(),
    );
    let path = audit_path("missing-decision-id");
    let mut audit = open_audit(&path);
    let mut repository = SecurityGovernanceRepository::default();
    assert_eq!(
        evaluate_with_trusted_workload(
            "policy_test",
            &mut repository,
            &command(SecurityGovernanceAction::RecordAudit),
            &policy,
            &mut audit,
        ),
        Err(TrpgError::PolicyEvidenceUntrusted)
    );
    assert_eq!(
        audit.verify().unwrap()[0].decision,
        AuditDecision::Unavailable
    );
    cleanup_audit(&path);

    let openfga = one_shot_policy_response(
        r#"{"allowed":true}"#,
        "X-Request-Id: openfga-decision-1\r\n",
    );
    let wrong_revision = one_shot_policy_response(
        r#"{"result":{"allow":true,"decision_id":"opa-decision-1","policy_revision":"wrong-revision"}}"#,
        "",
    );
    let policy = adapter(
        openfga,
        "/stores/test/check".to_owned(),
        "model-test".to_owned(),
        wrong_revision,
        "opa-security-governance-v2".to_owned(),
    );
    let path = audit_path("wrong-policy-revision");
    let mut audit = open_audit(&path);
    assert_eq!(
        evaluate_with_trusted_workload(
            "policy_test",
            &mut SecurityGovernanceRepository::default(),
            &command(SecurityGovernanceAction::RecordAudit),
            &policy,
            &mut audit,
        ),
        Err(TrpgError::PolicyEvidenceUntrusted)
    );
    cleanup_audit(&path);
}

#[test]
fn openfga_request_does_not_recreate_the_callers_role_as_a_contextual_tuple() {
    let (openfga, captured_request) = one_shot_policy_response_with_capture(
        r#"{"allowed":true}"#,
        "X-Request-Id: openfga-decision-no-self-grant\r\n",
    );
    let opa = one_shot_policy_response(
        r#"{"result":{"allow":true,"decision_id":"opa-decision-no-self-grant","policy_revision":"opa-security-governance-v2"}}"#,
        "",
    );
    let policy = adapter(
        openfga,
        "/stores/test/check".to_owned(),
        "model-test".to_owned(),
        opa,
        "opa-security-governance-v2".to_owned(),
    );
    let path = audit_path("no-contextual-self-grant");
    let mut audit = open_audit(&path);
    evaluate_with_trusted_workload(
        "policy_test",
        &mut SecurityGovernanceRepository::default(),
        &command(SecurityGovernanceAction::RecordAudit),
        &policy,
        &mut audit,
    )
    .unwrap();

    let request = captured_request
        .recv_timeout(Duration::from_secs(2))
        .expect("capture OpenFGA request");
    let body_start = request
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .expect("HTTP request boundary")
        + 4;
    let body: serde_json::Value = serde_json::from_slice(&request[body_start..]).unwrap();
    assert!(body.get("contextual_tuples").is_none());
    assert_eq!(
        body["tuple_key"]["relation"],
        serde_json::Value::String("can_record_audit".to_owned())
    );
    cleanup_audit(&path);
}

#[test]
fn formal_commit_authorizer_records_real_deny_and_never_synthesizes_a_permit() {
    let openfga = one_shot_policy_response(
        r#"{"allowed":false}"#,
        "X-Request-Id: openfga-formal-deny\r\n",
    );
    let opa = one_shot_policy_response(
        r#"{"result":{"allow":false,"decision_id":"opa-formal-deny","policy_revision":"opa-formal-v1"}}"#,
        "",
    );
    let policy = adapter(
        openfga,
        "/stores/test/check".to_owned(),
        "openfga-formal-v1".to_owned(),
        opa,
        "opa-formal-v1".to_owned(),
    );
    let contract =
        trpg_test_support::authority_contract("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    let (verifier, authentication) =
        trpg_test_support::formal_commit_identity_for_contract(&contract);
    let command = trpg_test_support::governed_command_for_contract(
        &contract,
        "formal decision",
        ActorRole::Workflow,
    );
    let path = audit_path("formal-deny");
    let audit = open_formal_audit(&path);
    let authorizer = FormalCommitAuthorizer::new(verifier, policy, audit.clone());

    assert_eq!(
        authorizer.authorize(&authentication, None, &command, "workflow", 2),
        Err(TrpgError::PolicyDenied)
    );
    let records = audit.verify().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].decision, AuditDecision::Deny);
    assert_eq!(records[0].action, "write_official_state");
    assert_eq!(records[0].openfga_decision_id, "openfga-formal-deny");
    assert_eq!(records[0].opa_decision_id, "opa-formal-deny");
    cleanup_audit(&path);
}

#[test]
fn formal_commit_authorizer_fails_closed_when_policy_is_unavailable() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let unavailable = listener.local_addr().unwrap();
    drop(listener);
    let policy = adapter(
        unavailable,
        "/stores/unavailable/check".to_owned(),
        "openfga-formal-unavailable".to_owned(),
        unavailable,
        "opa-formal-unavailable".to_owned(),
    );
    let contract =
        trpg_test_support::authority_contract("camp_ai_harbor", AuthorityMode::AiKp, 1).unwrap();
    let (verifier, authentication) =
        trpg_test_support::formal_commit_identity_for_contract(&contract);
    let command = trpg_test_support::governed_command_for_contract(
        &contract,
        "formal decision",
        ActorRole::Workflow,
    );
    let path = audit_path("formal-unavailable");
    let audit = open_formal_audit(&path);
    let authorizer = FormalCommitAuthorizer::new(verifier, policy, audit.clone());

    assert_eq!(
        authorizer.authorize(&authentication, None, &command, "workflow", 2),
        Err(TrpgError::PolicyUnavailable)
    );
    let records = audit.verify().unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].decision, AuditDecision::Unavailable);
    cleanup_audit(&path);
}

#[test]
fn real_openfga_and_opa_enforce_permit_and_visibility_deny() {
    let policy = real_policy_adapter()
        .expect("P02_OPENFGA_* and P02_OPA_ADDRESS must identify real policy services");

    let permit_path = audit_path("real-permit");
    let mut permit_audit = open_audit(&permit_path);
    let mut permit_repository = SecurityGovernanceRepository::default();
    let permit_command = command(SecurityGovernanceAction::RecordAudit);
    let event = evaluate_with_trusted_workload(
        "policy_real_e2e",
        &mut permit_repository,
        &permit_command,
        &policy,
        &mut permit_audit,
    )
    .unwrap();
    assert_eq!(event.authenticated_actor.id().as_str(), "workflow_001");
    let permit_records = permit_audit.verify().unwrap();
    assert_eq!(permit_records.len(), 1);
    assert_eq!(permit_records[0].decision, AuditDecision::Permit);
    assert!(permit_records[0].record_hash.starts_with("hmac-sha256:"));

    let deny_path = audit_path("real-deny");
    let mut deny_audit = open_audit(&deny_path);
    let mut deny_repository = SecurityGovernanceRepository::default();
    let mut deny_command = command(SecurityGovernanceAction::ExportPlayerReport);
    deny_command.payload.target_visibility = Visibility::new(VisibilityLabel::AiInternal);
    assert_eq!(
        evaluate_with_trusted_workload(
            "policy_real_e2e",
            &mut deny_repository,
            &deny_command,
            &policy,
            &mut deny_audit,
        )
        .unwrap_err(),
        TrpgError::PolicyDenied
    );
    assert!(deny_repository.events().is_empty());
    assert_eq!(
        deny_audit.verify().unwrap()[0].decision,
        AuditDecision::Deny
    );

    cleanup_audit(&permit_path);
    cleanup_audit(&deny_path);
}

fn audit_draft(index: usize) -> AuditRecordDraft {
    AuditRecordDraft {
        actor_id: format!("actor_{index}"),
        actor_origin: "workload".to_owned(),
        authentication_reference: format!("workload_{index}"),
        campaign_id: "campaign_a".to_owned(),
        resource_type: "campaign".to_owned(),
        resource_id: "campaign_a".to_owned(),
        action: "record_audit".to_owned(),
        requested_role: "not_applicable".to_owned(),
        visibility_label: "system_only".to_owned(),
        visibility_subject: "not_applicable".to_owned(),
        provenance_kind: "system_fixture".to_owned(),
        provenance_reference: format!("audit_fact_{index}"),
        provenance_recorded_by: "policy_test".to_owned(),
        decision: AuditDecision::Permit,
        openfga_decision_id: format!("openfga_{index}"),
        openfga_policy_revision: "openfga_model_1".to_owned(),
        opa_decision_id: format!("opa_{index}"),
        opa_policy_revision: "opa_bundle_1".to_owned(),
        trace_id: format!("trace_{index}"),
    }
}

#[test]
fn keyed_audit_chain_rejects_tampering_wrong_keys_and_serializes_concurrent_writers() {
    let path = audit_path("hmac-concurrency");
    let first = open_audit(&path);
    let second = FileAuditLog::open(&path, "test-audit-key-v1", &AUDIT_KEY).unwrap();
    let path_for_first = path.clone();
    let first_worker = thread::spawn(move || {
        let mut log = first;
        for index in 0..10 {
            log.append(audit_draft(index)).unwrap();
        }
        path_for_first
    });
    let second_worker = thread::spawn(move || {
        let mut log = second;
        for index in 10..20 {
            log.append(audit_draft(index)).unwrap();
        }
    });
    first_worker.join().unwrap();
    second_worker.join().unwrap();

    let records = FileAuditLog::open(&path, "test-audit-key-v1", &AUDIT_KEY)
        .unwrap()
        .verify()
        .unwrap();
    assert_eq!(records.len(), 20);
    assert_eq!(records.last().unwrap().sequence, 20);
    assert_eq!(
        FileAuditLog::open(&path, "test-audit-key-v1", &[0x24; 32]).unwrap_err(),
        TrpgError::AuditIntegrityViolation
    );

    let contents = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, contents.replace("actor_0", "actor_forged")).unwrap();
    assert_eq!(
        FileAuditLog::open(&path, "test-audit-key-v1", &AUDIT_KEY).unwrap_err(),
        TrpgError::AuditIntegrityViolation
    );
    cleanup_audit(&path);
}

#[test]
fn an_open_audit_log_detects_wholesale_file_deletion() {
    let path = audit_path("deleted");
    let mut audit = open_audit(&path);
    audit.append(audit_draft(1)).unwrap();
    std::fs::remove_file(&path).unwrap();

    assert_eq!(
        audit.verify().unwrap_err(),
        TrpgError::AuditIntegrityViolation
    );
    cleanup_audit(&path);
}

#[test]
fn durable_head_detects_wholesale_deletion_after_reopen() {
    let path = audit_path("deleted-after-reopen");
    let mut audit = open_audit(&path);
    audit.append(audit_draft(1)).unwrap();
    drop(audit);
    std::fs::remove_file(&path).unwrap();

    assert_eq!(
        FileAuditLog::open(&path, "test-audit-key-v1", &AUDIT_KEY).unwrap_err(),
        TrpgError::AuditIntegrityViolation
    );
    cleanup_audit(&path);
}
