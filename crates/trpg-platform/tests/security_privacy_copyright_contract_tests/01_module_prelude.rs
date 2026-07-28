use std::net::{SocketAddr, TcpListener};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use trpg_platform::security_privacy_copyright::{
    request_data_deletion, request_data_deletion_canonical,
    review_security_privacy_copyright_policy, ExportAudience, ExportIntent, RequestDataDeletion,
    ReviewSecurityPrivacyCopyrightPolicy, SecurityPrivacyCopyrightEvent,
    SecurityPrivacyCopyrightEventEnvelope, SecurityPrivacyCopyrightRepository,
    DATA_DELETION_REQUESTED_EVENT, SECURITY_PRIVACY_COPYRIGHT_METRIC_MODULE,
    SECURITY_PRIVACY_COPYRIGHT_REQUIRED_METRICS, SECURITY_PRIVACY_COPYRIGHT_REVIEWED_EVENT,
};
use trpg_security_governance::formal_commit_audit::{FormalCommitAudit, FormalCommitAuthorizer};
use trpg_security_governance::policy_adapter::{
    HttpPolicyEndpoint, OpenFgaOpaPolicyAdapter, PolicyBackend,
};
use trpg_security_governance::security_privacy::{
    ConfirmedDeletionRecord, DeletionEvidenceStatus, DeletionJob, DeletionJobStatus,
    DeletionRequestEvidence, DeletionRequestPort, PrivacyError,
};
use trpg_shared_kernel::{
    ActorRole, AuthorityContract, AuthorityMode, CommandEnvelope, EntityId, FormalWritePath,
    KernelResult, PrincipalScope, TrpgError, Visibility, VisibilityLabel,
};

static NEXT_AUDIT_ID: AtomicU64 = AtomicU64::new(1);

fn contract() -> AuthorityContract {
    trpg_test_support::authority_contract("camp_human_archive", AuthorityMode::HumanKp, 1)
        .expect("valid platform privacy authority contract")
}

fn audit() -> FormalCommitAudit {
    let id = NEXT_AUDIT_ID.fetch_add(1, Ordering::Relaxed);
    FormalCommitAudit::open(
        std::env::temp_dir().join(format!(
            "p05-platform-privacy-audit-{}-{id}.jsonl",
            std::process::id()
        )),
        "p05-platform-privacy-audit-key",
        &[0x73; 32],
    )
    .expect("open dedicated platform privacy audit")
}

fn custody_with_policy(
    policy: OpenFgaOpaPolicyAdapter,
) -> (FormalCommitAuthorizer, trpg_identity::AuthenticationContext) {
    let contract = contract();
    let (identity, authentication) =
        trpg_test_support::formal_commit_identity_for_contract(&contract);
    (
        FormalCommitAuthorizer::new(identity, policy, audit()),
        authentication,
    )
}

fn permitted_custody() -> (FormalCommitAuthorizer, trpg_identity::AuthenticationContext) {
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
    custody_with_policy(policy)
}

fn permitted_deletion_custody() -> (
    FormalCommitAuthorizer,
    trpg_identity::AuthenticationContext,
    trpg_identity::AuthenticationContext,
) {
    use trpg_identity::{CampaignRole, WorkloadRole};

    let contract = contract();
    let mut identity = trpg_test_support::identity_service_for_contract(&contract);
    let session = identity
        .login(
            "test-authority-registrar@example.test",
            "test authority password long enough",
            200,
        )
        .unwrap();
    let requester = identity
        .authenticate_session(Some(session.token.expose()), 201)
        .unwrap();
    identity
        .grant_membership(
            &requester,
            contract.campaign_id().as_str(),
            requester.subject_id().as_str(),
            CampaignRole::CampaignOwner,
            201,
        )
        .unwrap();
    let credential = identity
        .issue_workload_credential("workflow_001", WorkloadRole::WorkflowEngine, 1, u64::MAX)
        .unwrap();
    let workflow = identity.authenticate_workload(&credential, 2).unwrap();
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
    (
        FormalCommitAuthorizer::new(identity.verifier(), policy, audit()),
        workflow,
        requester,
    )
}

fn real_deletion_custody(
    workflow_id: &str,
    workload_role: trpg_identity::WorkloadRole,
) -> (
    FormalCommitAuthorizer,
    trpg_identity::AuthenticationContext,
    trpg_identity::AuthenticationContext,
) {
    use trpg_identity::CampaignRole;

    let contract = contract();
    let mut identity = trpg_test_support::identity_service_for_contract(&contract);
    let session = identity
        .login(
            "test-authority-registrar@example.test",
            "test authority password long enough",
            200,
        )
        .unwrap();
    let requester = identity
        .authenticate_session(Some(session.token.expose()), 201)
        .unwrap();
    identity
        .grant_membership(
            &requester,
            contract.campaign_id().as_str(),
            requester.subject_id().as_str(),
            CampaignRole::CampaignOwner,
            201,
        )
        .unwrap();
    let credential = identity
        .issue_workload_credential(workflow_id, workload_role, 1, u64::MAX)
        .unwrap();
    let workflow = identity.authenticate_workload(&credential, 2).unwrap();
    let openfga_address = std::env::var("P02_OPENFGA_ADDRESS")
        .expect("P02_OPENFGA_ADDRESS must identify the real policy service")
        .parse()
        .expect("valid P02 OpenFGA address");
    let openfga_store =
        std::env::var("P02_OPENFGA_STORE_ID").expect("P02_OPENFGA_STORE_ID is required");
    let openfga_model =
        std::env::var("P02_OPENFGA_MODEL_ID").expect("P02_OPENFGA_MODEL_ID is required");
    let opa_address = std::env::var("P02_OPA_ADDRESS")
        .expect("P02_OPA_ADDRESS must identify the real policy service")
        .parse()
        .expect("valid P02 OPA address");
    let opa_revision = std::env::var("P02_OPA_REVISION")
        .unwrap_or_else(|_| "opa-security-governance-v3".to_owned());
    let policy = OpenFgaOpaPolicyAdapter::new(
        HttpPolicyEndpoint::new(
            openfga_address,
            format!("/stores/{openfga_store}/check"),
            PolicyBackend::OpenFga,
            openfga_model,
        )
        .unwrap(),
        HttpPolicyEndpoint::new(
            opa_address,
            "/v1/data/security_governance/decision",
            PolicyBackend::Opa,
            opa_revision,
        )
        .unwrap(),
    )
    .unwrap();
    (
        FormalCommitAuthorizer::new(identity.verifier(), policy, audit()),
        workflow,
        requester,
    )
}

fn deletion_custody_without_campaign_membership() -> (
    FormalCommitAuthorizer,
    trpg_identity::AuthenticationContext,
    trpg_identity::AuthenticationContext,
) {
    use trpg_identity::WorkloadRole;

    let contract = contract();
    let mut identity = trpg_test_support::identity_service_for_contract(&contract);
    let session = identity
        .login(
            "test-authority-registrar@example.test",
            "test authority password long enough",
            200,
        )
        .unwrap();
    let requester = identity
        .authenticate_session(Some(session.token.expose()), 201)
        .unwrap();
    let credential = identity
        .issue_workload_credential("workflow_001", WorkloadRole::WorkflowEngine, 1, u64::MAX)
        .unwrap();
    let workflow = identity.authenticate_workload(&credential, 2).unwrap();
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
    (
        FormalCommitAuthorizer::new(identity.verifier(), policy, audit()),
        workflow,
        requester,
    )
}

fn unused_loopback_address() -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").expect("reserve loopback address");
    listener.local_addr().expect("loopback address")
}

fn unavailable_custody() -> (FormalCommitAuthorizer, trpg_identity::AuthenticationContext) {
    let openfga = unused_loopback_address();
    let opa = unused_loopback_address();
    let policy = OpenFgaOpaPolicyAdapter::new(
        HttpPolicyEndpoint::new(
            openfga,
            "/stores/test/check",
            PolicyBackend::OpenFga,
            "unavailable-openfga-v1",
        )
        .unwrap(),
        HttpPolicyEndpoint::new(
            opa,
            "/v1/data/security_governance/decision",
            PolicyBackend::Opa,
            "unavailable-opa-v1",
        )
        .unwrap(),
    )
    .unwrap();
    custody_with_policy(policy)
}

fn execute_review(
    repository: &mut SecurityPrivacyCopyrightRepository,
    command: &CommandEnvelope<ReviewSecurityPrivacyCopyrightPolicy>,
) -> KernelResult<SecurityPrivacyCopyrightEventEnvelope> {
    let (authorizer, authentication) = permitted_custody();
    review_security_privacy_copyright_policy(
        repository,
        &authorizer,
        &authentication,
        None,
        command,
        2,
    )
}

fn review_command() -> CommandEnvelope<ReviewSecurityPrivacyCopyrightPolicy> {
    let mut command = trpg_test_support::governed_command_for_contract(
        &contract(),
        ReviewSecurityPrivacyCopyrightPolicy {
            asset_id: "handout_001".to_owned(),
            license_tag: "original_campaign_asset".to_owned(),
            detail: "keeper_only_handout_notes".to_owned(),
            export_intent: ExportIntent::ExportTo(ExportAudience::Public),
        },
        ActorRole::Workflow,
    );
    command.visibility = Visibility::new(VisibilityLabel::Public);
    command
}

fn deletion_command() -> CommandEnvelope<RequestDataDeletion> {
    trpg_test_support::governed_command_for_contract(
        &contract(),
        RequestDataDeletion {
            job_id: "deletion_001".to_owned(),
            subject_id: "player_001".to_owned(),
            retention_policy: "audit_log_retained_private_payload_removed".to_owned(),
            reason: "player privacy request".to_owned(),
        },
        ActorRole::Workflow,
    )
}

#[derive(Default)]
struct RecordingDeletionPort {
    called: AtomicBool,
    confirmed: AtomicBool,
    requested_by: Mutex<Option<String>>,
}

#[derive(Debug)]
struct ForgedHashCanonicalPort {
    inner: Arc<dyn trpg_shared_kernel::CanonicalCommitPort>,
}
