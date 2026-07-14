use std::net::{SocketAddr, TcpListener};
use std::path::PathBuf;
use std::thread;

use trpg_security_governance::policy_adapter::{
    HttpPolicyEndpoint, OpenFgaOpaPolicyAdapter, PolicyBackend,
};
use trpg_security_governance::tamper_evident_audit::{
    AuditDecision, AuditRecordDraft, AuditSink, FileAuditLog,
};
use trpg_security_governance::{
    evaluate_security_governance, evaluate_security_governance_with_policy,
    SecurityGovernanceAction, SecurityGovernanceCommand, SecurityGovernanceRepository,
};
use trpg_shared_kernel::{ActorRole, AuthorityMode, TrpgError, Visibility, VisibilityLabel};

const AUDIT_KEY: [u8; 32] = [0x42; 32];

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

fn open_audit(path: &PathBuf) -> FileAuditLog {
    let _ = std::fs::remove_file(path);
    FileAuditLog::open(path, "test-audit-key-v1", &AUDIT_KEY).unwrap()
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
            "/v1/data/security_governance/allow",
            PolicyBackend::Opa,
            opa_revision,
        )
        .unwrap(),
    )
    .unwrap()
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
        .unwrap_or_else(|_| "opa-security-governance-v1".to_owned());
    Some(adapter(
        openfga,
        format!("/stores/{store_id}/check"),
        model_id,
        opa,
        opa_revision,
    ))
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
        evaluate_security_governance_with_policy(
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
    std::fs::remove_file(path).unwrap();
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
        evaluate_security_governance_with_policy(
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
    std::fs::remove_file(path).unwrap();
}

#[test]
fn real_openfga_and_opa_enforce_permit_and_visibility_deny() {
    let policy = real_policy_adapter()
        .expect("P02_OPENFGA_* and P02_OPA_ADDRESS must identify real policy services");

    let permit_path = audit_path("real-permit");
    let mut permit_audit = open_audit(&permit_path);
    let mut permit_repository = SecurityGovernanceRepository::default();
    let permit_command = command(SecurityGovernanceAction::RecordAudit);
    let event = evaluate_security_governance_with_policy(
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
        evaluate_security_governance_with_policy(
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

    std::fs::remove_file(permit_path).unwrap();
    std::fs::remove_file(deny_path).unwrap();
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
    std::fs::remove_file(path).unwrap();
}
